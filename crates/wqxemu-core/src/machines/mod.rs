// Hardware machine implementations.
//
// Each supported model lives in its own module and implements the
// `Machine` trait. `create_machine` instantiates a model from a
// `MachineModel`, and `detect_model` guesses the model from the ROM
// files when the frontend does not specify one explicitly.

use anyhow::Result;

use crate::machine::{Machine, MachineModel, RomFiles};

pub mod cc800;
pub mod nc1020;
pub mod nc2000;
pub mod pc1000;

/// Create a machine for the given model, loading its ROM files.
pub fn create_machine(model: MachineModel, files: &RomFiles) -> Result<Box<dyn Machine>> {
    match model {
        MachineModel::Cc800 => Ok(Box::new(cc800::Cc800Machine::new(files)?)),
        MachineModel::Nc1020 => Ok(Box::new(nc1020::Nc1020Machine::new(files)?)),
        MachineModel::Pc1000 => Ok(Box::new(pc1000::Pc1000Machine::new(files)?)),
        MachineModel::Nc2000 => Ok(Box::new(nc2000::Nc2000Machine::new(files)?)),
    }
}

/// Guess the machine model from the ROM files.
///
/// Heuristics:
/// - NC2000 requires a NAND dump (`*.nand`), which no other model uses.
/// - NC1020 system ROMs are 24MB dumps (`obj_lu.bin`).
/// - PC1000 system ROMs are 12MB dumps (`pc1000.rom`).
/// - CC800 system ROMs are 16MB dumps (`obj.bin`).
pub fn detect_model(files: &RomFiles) -> MachineModel {
    if files.nand.is_some() || files.nand0.is_some() {
        return MachineModel::Nc2000;
    }

    if let Some(rom) = &files.rom {
        if let Ok(meta) = std::fs::metadata(rom) {
            let len = meta.len();
            if len == 24 * 1024 * 1024 {
                // NC1020 uses 24MB (3 x 8MB volume) ROM dumps.
                return MachineModel::Nc1020;
            }
            if len == 12 * 1024 * 1024 || len == 16 * 1024 * 1024 {
                if len == 12 * 1024 * 1024 {
                    // PC1000 uses 12MB (obj1 + obj2 + obj3).
                    return MachineModel::Pc1000;
                }
                // Both PC1000 (16MB Android/PC1000EMUX layout) and CC800
                // (16MB obj.bin) dumps are 16MB. The CC800's volume 1
                // starts at offset 8MB with a copy of the boot page,
                // while the PC1000 Android layout leaves 8-12MB empty.
                if let Ok(data) = std::fs::read(rom) {
                    if data.len() >= 0x800000 + 6
                        && data[0x800000..0x800006] == [0x4C, 0x4A, 0x88, 0xEA, 0x60, 0xEA]
                    {
                        return MachineModel::Cc800;
                    }
                }
                return MachineModel::Pc1000;
            }
        }
    }

    MachineModel::Nc1020
}
