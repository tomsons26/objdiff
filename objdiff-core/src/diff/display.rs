use std::cmp::Ordering;

use anyhow::{bail, Result};

use crate::obj::{
    ObjInsArg, ObjInsArgDiff, ObjInsArgValue, ObjInsDiff, ObjReloc, ObjRelocKind, ObjSymbol,
};

#[derive(Debug, Clone)]
pub enum DiffText<'a> {
    /// Basic text
    Basic(&'a str),
    /// Colored text
    BasicColor(&'a str, usize),
    /// Line number
    Line(usize),
    /// Instruction address
    Address(u32),
    /// Instruction mnemonic
    Opcode(&'a str, u8),
    /// Instruction argument
    Argument(&'a ObjInsArgValue, Option<&'a ObjInsArgDiff>),
    /// Branch target
    BranchTarget(u32),
    /// Symbol name
    Symbol(&'a ObjSymbol),
    /// Number of spaces
    Spacing(usize),
    /// End of line
    Eol,
}

pub fn display_diff(
    ins_diff: &ObjInsDiff,
    base_addr: u32,
    mut cb: impl FnMut(DiffText) -> Result<()>,
) -> Result<()> {
    let Some(ins) = &ins_diff.ins else {
        cb(DiffText::Eol)?;
        return Ok(());
    };
    if let Some(line) = ins.line {
        cb(DiffText::Line(line as usize))?;
    }
    cb(DiffText::Address(ins.address - base_addr))?;
    if let Some(branch) = &ins_diff.branch_from {
        cb(DiffText::BasicColor(" ~> ", branch.branch_idx))?;
    } else {
        cb(DiffText::Spacing(4))?;
    }
    cb(DiffText::Opcode(&ins.mnemonic, ins.op))?;
    let mut writing_offset = false;
    for (i, arg) in ins.args.iter().enumerate() {
        if i == 0 {
            cb(DiffText::Spacing(1))?;
        }
        if i > 0 && !writing_offset {
            cb(DiffText::Basic(", "))?;
        }
        let mut new_writing_offset = false;
        match arg {
            ObjInsArg::Arg(v) => {
                let diff = ins_diff.arg_diff.get(i).and_then(|o| o.as_ref());
                cb(DiffText::Argument(v, diff))?;
            }
            ObjInsArg::ArgWithBase(v) => {
                let diff = ins_diff.arg_diff.get(i).and_then(|o| o.as_ref());
                cb(DiffText::Argument(v, diff))?;
                cb(DiffText::Basic("("))?;
                new_writing_offset = true;
            }
            ObjInsArg::Reloc => {
                display_reloc(ins.reloc.as_ref().unwrap(), &mut cb)?;
            }
            ObjInsArg::RelocWithBase => {
                display_reloc(ins.reloc.as_ref().unwrap(), &mut cb)?;
                cb(DiffText::Basic("("))?;
                new_writing_offset = true;
            }
            ObjInsArg::BranchOffset(offset) => {
                let addr = offset + ins.address as i32 - base_addr as i32;
                cb(DiffText::BranchTarget(addr as u32))?;
            }
        }
        if writing_offset {
            cb(DiffText::Basic(")"))?;
        }
        writing_offset = new_writing_offset;
    }
    if let Some(branch) = &ins_diff.branch_to {
        cb(DiffText::BasicColor(" ~>", branch.branch_idx))?;
    }
    cb(DiffText::Eol)?;
    Ok(())
}

fn display_reloc_name(reloc: &ObjReloc, mut cb: impl FnMut(DiffText) -> Result<()>) -> Result<()> {
    cb(DiffText::Symbol(&reloc.target))?;
    match reloc.target.addend.cmp(&0i64) {
        Ordering::Greater => cb(DiffText::Basic(&format!("+{:#X}", reloc.target.addend))),
        Ordering::Less => cb(DiffText::Basic(&format!("-{:#X}", -reloc.target.addend))),
        _ => Ok(()),
    }
}

fn display_reloc(reloc: &ObjReloc, mut cb: impl FnMut(DiffText) -> Result<()>) -> Result<()> {
    match reloc.kind {
        #[cfg(feature = "ppc")]
        ObjRelocKind::PpcAddr16Lo => {
            display_reloc_name(reloc, &mut cb)?;
            cb(DiffText::Basic("@l"))?;
        }
        #[cfg(feature = "ppc")]
        ObjRelocKind::PpcAddr16Hi => {
            display_reloc_name(reloc, &mut cb)?;
            cb(DiffText::Basic("@h"))?;
        }
        #[cfg(feature = "ppc")]
        ObjRelocKind::PpcAddr16Ha => {
            display_reloc_name(reloc, &mut cb)?;
            cb(DiffText::Basic("@ha"))?;
        }
        #[cfg(feature = "ppc")]
        ObjRelocKind::PpcEmbSda21 => {
            display_reloc_name(reloc, &mut cb)?;
            cb(DiffText::Basic("@sda21"))?;
        }
        #[cfg(feature = "ppc")]
        ObjRelocKind::PpcRel24 | ObjRelocKind::PpcRel14 => {
            display_reloc_name(reloc, &mut cb)?;
        }
        #[cfg(feature = "mips")]
        ObjRelocKind::MipsHi16 => {
            cb(DiffText::Basic("%hi("))?;
            display_reloc_name(reloc, &mut cb)?;
            cb(DiffText::Basic(")"))?;
        }
        #[cfg(feature = "mips")]
        ObjRelocKind::MipsLo16 => {
            cb(DiffText::Basic("%lo("))?;
            display_reloc_name(reloc, &mut cb)?;
            cb(DiffText::Basic(")"))?;
        }
        #[cfg(feature = "mips")]
        ObjRelocKind::MipsGot16 => {
            cb(DiffText::Basic("%got("))?;
            display_reloc_name(reloc, &mut cb)?;
            cb(DiffText::Basic(")"))?;
        }
        #[cfg(feature = "mips")]
        ObjRelocKind::MipsCall16 => {
            cb(DiffText::Basic("%call16("))?;
            display_reloc_name(reloc, &mut cb)?;
            cb(DiffText::Basic(")"))?;
        }
        #[cfg(feature = "mips")]
        ObjRelocKind::MipsGpRel16 => {
            cb(DiffText::Basic("%gp_rel("))?;
            display_reloc_name(reloc, &mut cb)?;
            cb(DiffText::Basic(")"))?;
        }
        #[cfg(feature = "mips")]
        ObjRelocKind::Mips26 => {
            display_reloc_name(reloc, &mut cb)?;
        }
        #[cfg(feature = "mips")]
        ObjRelocKind::MipsGpRel32 => {
            bail!("unimplemented: mips gp_rel32");
        }
        ObjRelocKind::Absolute => {
            cb(DiffText::Basic("[INVALID]"))?;
        }
    }
    Ok(())
}
