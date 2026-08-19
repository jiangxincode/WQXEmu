// Hardware machine implementations.
//
// Each supported model lives in its own module and implements the
// `Machine` trait. `create_machine` instantiates a model from a
// `MachineModel`, and `detect_model` guesses the model from the ROM
// files when the frontend does not specify one explicitly.

use anyhow::Result;

use crate::machine::{Machine, MachineModel, RomFiles};

pub mod nc1020;
pub mod nc2000;
pub mod pc1000;

/// Create a machine for the given model, loading its ROM files.
pub fn create_machine(model: MachineModel, files: &RomFiles) -> Result<Box<dyn Machine>> {
    match model {
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
                // PC1000 uses 12MB (obj1 + obj2 + obj3) or the 16MB
                // Android/PC1000EMUX buffer layout.
                return MachineModel::Pc1000;
            }
        }
    }

    MachineModel::Nc1020
}
