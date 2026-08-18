// 6502 CPU emulation for Wenquxing NC1020 (SPDC1024 SoC).
// Implements the full 6502 instruction set with cycle-accurate timing.

use serde::{Deserialize, Serialize};

/// CPU status flags
pub const FLAG_C: u8 = 0x01; // Carry
pub const FLAG_Z: u8 = 0x02; // Zero
pub const FLAG_I: u8 = 0x04; // Interrupt disable
pub const FLAG_D: u8 = 0x08; // Decimal mode
pub const FLAG_B: u8 = 0x10; // Break
pub const FLAG_U: u8 = 0x20; // Unused (always 1 when pushed)
pub const FLAG_V: u8 = 0x40; // Overflow
pub const FLAG_N: u8 = 0x80; // Negative

/// NMI vector address
pub const NMI_VECTOR: u16 = 0xFFFA;
/// Reset vector address
pub const RESET_VECTOR: u16 = 0xFFFC;
/// IRQ vector address
pub const IRQ_VECTOR: u16 = 0xFFFE;

/// 6502 CPU state
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Cpu {
    /// Program counter
    pub pc: u16,
    /// Accumulator
    pub a: u8,
    /// X index register
    pub x: u8,
    /// Y index register
    pub y: u8,
    /// Stack pointer
    pub sp: u8,
    /// Processor status
    pub ps: u8,
    /// Total cycles executed
    pub cycles: u64,
    /// Whether IRQ is pending
    pub irq_pending: bool,
    /// Whether NMI is pending
    pub nmi_pending: bool,
}

impl Cpu {
    /// Create a new CPU in reset state
    pub fn new() -> Self {
        Self {
            pc: 0,
            a: 0,
            x: 0,
            y: 0,
            sp: 0xFF,
            ps: FLAG_I | FLAG_U,
            cycles: 0,
            irq_pending: false,
            nmi_pending: false,
        }
    }

    /// Reset the CPU to initial state
    pub fn reset(&mut self, reset_vector: u16) {
        self.a = 0;
        self.x = 0;
        self.y = 0;
        self.sp = 0xFF;
        self.ps = FLAG_I | FLAG_U;
        self.pc = reset_vector;
        self.irq_pending = false;
        self.nmi_pending = false;
    }

    /// Get the N flag (negative)
    #[inline]
    pub fn flag_n(&self) -> bool {
        self.ps & FLAG_N != 0
    }

    /// Get the V flag (overflow)
    #[inline]
    pub fn flag_v(&self) -> bool {
        self.ps & FLAG_V != 0
    }

    /// Get the D flag (decimal)
    #[inline]
    pub fn flag_d(&self) -> bool {
        self.ps & FLAG_D != 0
    }

    /// Get the I flag (interrupt disable)
    #[inline]
    pub fn flag_i(&self) -> bool {
        self.ps & FLAG_I != 0
    }

    /// Get the Z flag (zero)
    #[inline]
    pub fn flag_z(&self) -> bool {
        self.ps & FLAG_Z != 0
    }

    /// Get the C flag (carry)
    #[inline]
    pub fn flag_c(&self) -> bool {
        self.ps & FLAG_C != 0
    }

    /// Set N and Z flags based on a value
    #[inline]
    fn set_nz(&mut self, value: u8) {
        self.ps = (self.ps & !(FLAG_N | FLAG_Z))
            | if value & 0x80 != 0 { FLAG_N } else { 0 }
            | if value == 0 { FLAG_Z } else { 0 };
    }

    /// Push a byte onto the stack
    #[inline]
    fn push(&mut self, bus: &mut impl CpuBus, value: u8) {
        bus.write(0x0100 | self.sp as u16, value);
        self.sp = self.sp.wrapping_sub(1);
    }

    /// Pop a byte from the stack
    #[inline]
    fn pop(&mut self, bus: &mut impl CpuBus) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        bus.read(0x0100 | self.sp as u16)
    }

    /// Handle NMI interrupt
    fn handle_nmi(&mut self, bus: &mut impl CpuBus) {
        self.push(bus, (self.pc >> 8) as u8);
        self.push(bus, (self.pc & 0xFF) as u8);
        self.push(bus, self.ps & !FLAG_B);
        self.ps |= FLAG_I;
        self.pc = bus.read_u16(NMI_VECTOR);
        self.nmi_pending = false;
        self.cycles += 7;
    }

    /// Handle IRQ interrupt
    fn handle_irq(&mut self, bus: &mut impl CpuBus) {
        self.push(bus, (self.pc >> 8) as u8);
        self.push(bus, (self.pc & 0xFF) as u8);
        self.push(bus, self.ps & !FLAG_B);
        self.ps |= FLAG_I;
        self.pc = bus.read_u16(IRQ_VECTOR);
        self.irq_pending = false;
        self.cycles += 7;
    }

    /// Execute one instruction. Returns the number of cycles consumed.
    pub fn step(&mut self, bus: &mut impl CpuBus) -> u64 {
        let start_cycles = self.cycles;

        // Check for pending interrupts
        if self.nmi_pending {
            self.handle_nmi(bus);
            return self.cycles - start_cycles;
        }
        if self.irq_pending && !self.flag_i() {
            self.handle_irq(bus);
            return self.cycles - start_cycles;
        }

        let opcode = bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);

        match opcode {
            // === ADC: Add with Carry ===
            0x69 => {
                let v = self.imm(bus);
                self.adc(v);
                self.cycles += 2;
            }
            0x65 => {
                let (a, _) = self.zp(bus);
                let v = self.read_bus(bus, a);
                self.adc(v);
                self.cycles += 3;
            }
            0x75 => {
                let (a, _) = self.zpx(bus);
                let v = self.read_bus(bus, a);
                self.adc(v);
                self.cycles += 4;
            }
            0x6D => {
                let (a, _) = self.abs(bus);
                let v = self.read_bus(bus, a);
                self.adc(v);
                self.cycles += 4;
            }
            0x7D => {
                let (a, p) = self.abx(bus);
                let v = self.read_bus(bus, a);
                self.adc(v);
                self.cycles += 4 + p as u64;
            }
            0x79 => {
                let (a, p) = self.aby(bus);
                let v = self.read_bus(bus, a);
                self.adc(v);
                self.cycles += 4 + p as u64;
            }
            0x61 => {
                let (a, _) = self.izx(bus);
                let v = self.read_bus(bus, a);
                self.adc(v);
                self.cycles += 6;
            }
            0x71 => {
                let (a, p) = self.izy(bus);
                let v = self.read_bus(bus, a);
                self.adc(v);
                self.cycles += 5 + p as u64;
            }

            // === AND: Logical AND ===
            0x29 => {
                let v = self.imm(bus);
                self.and(v);
                self.cycles += 2;
            }
            0x25 => {
                let (a, _) = self.zp(bus);
                let v = self.read_bus(bus, a);
                self.and(v);
                self.cycles += 3;
            }
            0x35 => {
                let (a, _) = self.zpx(bus);
                let v = self.read_bus(bus, a);
                self.and(v);
                self.cycles += 4;
            }
            0x2D => {
                let (a, _) = self.abs(bus);
                let v = self.read_bus(bus, a);
                self.and(v);
                self.cycles += 4;
            }
            0x3D => {
                let (a, p) = self.abx(bus);
                let v = self.read_bus(bus, a);
                self.and(v);
                self.cycles += 4 + p as u64;
            }
            0x39 => {
                let (a, p) = self.aby(bus);
                let v = self.read_bus(bus, a);
                self.and(v);
                self.cycles += 4 + p as u64;
            }
            0x21 => {
                let (a, _) = self.izx(bus);
                let v = self.read_bus(bus, a);
                self.and(v);
                self.cycles += 6;
            }
            0x31 => {
                let (a, p) = self.izy(bus);
                let v = self.read_bus(bus, a);
                self.and(v);
                self.cycles += 5 + p as u64;
            }

            // === ASL: Arithmetic Shift Left ===
            0x0A => {
                self.asl_acc();
                self.cycles += 2;
            }
            0x06 => {
                let (a, _) = self.zp(bus);
                self.asl_mem(bus, a);
                self.cycles += 5;
            }
            0x16 => {
                let (a, _) = self.zpx(bus);
                self.asl_mem(bus, a);
                self.cycles += 6;
            }
            0x0E => {
                let (a, _) = self.abs(bus);
                self.asl_mem(bus, a);
                self.cycles += 6;
            }
            0x1E => {
                let (a, _) = self.abx(bus);
                self.asl_mem(bus, a);
                self.cycles += 7;
            }

            // === BCC: Branch if Carry Clear ===
            0x90 => {
                let c = self.branch(bus, !self.flag_c());
                self.cycles += c;
            }

            // === BCS: Branch if Carry Set ===
            0xB0 => {
                let c = self.branch(bus, self.flag_c());
                self.cycles += c;
            }

            // === BEQ: Branch if Equal (Z set) ===
            0xF0 => {
                let c = self.branch(bus, self.flag_z());
                self.cycles += c;
            }

            // === BIT: Bit Test ===
            0x24 => {
                let (a, _) = self.zp(bus);
                self.bit(bus, a);
                self.cycles += 3;
            }
            0x2C => {
                let (a, _) = self.abs(bus);
                self.bit(bus, a);
                self.cycles += 4;
            }

            // === BMI: Branch if Minus (N set) ===
            0x30 => {
                let c = self.branch(bus, self.flag_n());
                self.cycles += c;
            }

            // === BNE: Branch if Not Equal (Z clear) ===
            0xD0 => {
                let c = self.branch(bus, !self.flag_z());
                self.cycles += c;
            }

            // === BPL: Branch if Plus (N clear) ===
            0x10 => {
                let c = self.branch(bus, !self.flag_n());
                self.cycles += c;
            }

            // === BRK: Force Interrupt ===
            0x00 => {
                self.pc = self.pc.wrapping_add(1);
                self.push(bus, (self.pc >> 8) as u8);
                self.push(bus, (self.pc & 0xFF) as u8);
                self.push(bus, self.ps | FLAG_B | FLAG_U);
                self.ps |= FLAG_I;
                self.pc = bus.read_u16(IRQ_VECTOR);
                self.cycles += 7;
            }

            // === BVC: Branch if Overflow Clear ===
            0x50 => {
                let c = self.branch(bus, !self.flag_v());
                self.cycles += c;
            }

            // === BVS: Branch if Overflow Set ===
            0x70 => {
                let c = self.branch(bus, self.flag_v());
                self.cycles += c;
            }

            // === CLC: Clear Carry ===
            0x18 => {
                self.ps &= !FLAG_C;
                self.cycles += 2;
            }

            // === CLD: Clear Decimal ===
            0xD8 => {
                self.ps &= !FLAG_D;
                self.cycles += 2;
            }

            // === CLI: Clear Interrupt Disable ===
            0x58 => {
                self.ps &= !FLAG_I;
                self.cycles += 2;
            }

            // === CLV: Clear Overflow ===
            0xB8 => {
                self.ps &= !FLAG_V;
                self.cycles += 2;
            }

            // === CMP: Compare ===
            0xC9 => {
                let v = self.imm(bus);
                self.cmp(self.a, v);
                self.cycles += 2;
            }
            0xC5 => {
                let (a, _) = self.zp(bus);
                let v = self.read_bus(bus, a);
                self.cmp(self.a, v);
                self.cycles += 3;
            }
            0xD5 => {
                let (a, _) = self.zpx(bus);
                let v = self.read_bus(bus, a);
                self.cmp(self.a, v);
                self.cycles += 4;
            }
            0xCD => {
                let (a, _) = self.abs(bus);
                let v = self.read_bus(bus, a);
                self.cmp(self.a, v);
                self.cycles += 4;
            }
            0xDD => {
                let (a, p) = self.abx(bus);
                let v = self.read_bus(bus, a);
                self.cmp(self.a, v);
                self.cycles += 4 + p as u64;
            }
            0xD9 => {
                let (a, p) = self.aby(bus);
                let v = self.read_bus(bus, a);
                self.cmp(self.a, v);
                self.cycles += 4 + p as u64;
            }
            0xC1 => {
                let (a, _) = self.izx(bus);
                let v = self.read_bus(bus, a);
                self.cmp(self.a, v);
                self.cycles += 6;
            }
            0xD1 => {
                let (a, p) = self.izy(bus);
                let v = self.read_bus(bus, a);
                self.cmp(self.a, v);
                self.cycles += 5 + p as u64;
            }

            // === CPX: Compare X ===
            0xE0 => {
                let v = self.imm(bus);
                self.cmp(self.x, v);
                self.cycles += 2;
            }
            0xE4 => {
                let (a, _) = self.zp(bus);
                let v = self.read_bus(bus, a);
                self.cmp(self.x, v);
                self.cycles += 3;
            }
            0xEC => {
                let (a, _) = self.abs(bus);
                let v = self.read_bus(bus, a);
                self.cmp(self.x, v);
                self.cycles += 4;
            }

            // === CPY: Compare Y ===
            0xC0 => {
                let v = self.imm(bus);
                self.cmp(self.y, v);
                self.cycles += 2;
            }
            0xC4 => {
                let (a, _) = self.zp(bus);
                let v = self.read_bus(bus, a);
                self.cmp(self.y, v);
                self.cycles += 3;
            }
            0xCC => {
                let (a, _) = self.abs(bus);
                let v = self.read_bus(bus, a);
                self.cmp(self.y, v);
                self.cycles += 4;
            }

            // === DEC: Decrement Memory ===
            0xC6 => {
                let (a, _) = self.zp(bus);
                self.dec(bus, a);
                self.cycles += 5;
            }
            0xD6 => {
                let (a, _) = self.zpx(bus);
                self.dec(bus, a);
                self.cycles += 6;
            }
            0xCE => {
                let (a, _) = self.abs(bus);
                self.dec(bus, a);
                self.cycles += 6;
            }
            0xDE => {
                let (a, _) = self.abx(bus);
                self.dec(bus, a);
                self.cycles += 7;
            }

            // === DEX: Decrement X ===
            0xCA => {
                self.x = self.x.wrapping_sub(1);
                self.set_nz(self.x);
                self.cycles += 2;
            }

            // === DEY: Decrement Y ===
            0x88 => {
                self.y = self.y.wrapping_sub(1);
                self.set_nz(self.y);
                self.cycles += 2;
            }

            // === EOR: Exclusive OR ===
            0x49 => {
                let v = self.imm(bus);
                self.eor(v);
                self.cycles += 2;
            }
            0x45 => {
                let (a, _) = self.zp(bus);
                let v = self.read_bus(bus, a);
                self.eor(v);
                self.cycles += 3;
            }
            0x55 => {
                let (a, _) = self.zpx(bus);
                let v = self.read_bus(bus, a);
                self.eor(v);
                self.cycles += 4;
            }
            0x4D => {
                let (a, _) = self.abs(bus);
                let v = self.read_bus(bus, a);
                self.eor(v);
                self.cycles += 4;
            }
            0x5D => {
                let (a, p) = self.abx(bus);
                let v = self.read_bus(bus, a);
                self.eor(v);
                self.cycles += 4 + p as u64;
            }
            0x59 => {
                let (a, p) = self.aby(bus);
                let v = self.read_bus(bus, a);
                self.eor(v);
                self.cycles += 4 + p as u64;
            }
            0x41 => {
                let (a, _) = self.izx(bus);
                let v = self.read_bus(bus, a);
                self.eor(v);
                self.cycles += 6;
            }
            0x51 => {
                let (a, p) = self.izy(bus);
                let v = self.read_bus(bus, a);
                self.eor(v);
                self.cycles += 5 + p as u64;
            }

            // === INC: Increment Memory ===
            0xE6 => {
                let (a, _) = self.zp(bus);
                self.inc(bus, a);
                self.cycles += 5;
            }
            0xF6 => {
                let (a, _) = self.zpx(bus);
                self.inc(bus, a);
                self.cycles += 6;
            }
            0xEE => {
                let (a, _) = self.abs(bus);
                self.inc(bus, a);
                self.cycles += 6;
            }
            0xFE => {
                let (a, _) = self.abx(bus);
                self.inc(bus, a);
                self.cycles += 7;
            }

            // === INX: Increment X ===
            0xE8 => {
                self.x = self.x.wrapping_add(1);
                self.set_nz(self.x);
                self.cycles += 2;
            }

            // === INY: Increment Y ===
            0xC8 => {
                self.y = self.y.wrapping_add(1);
                self.set_nz(self.y);
                self.cycles += 2;
            }

            // === JMP: Jump ===
            0x4C => {
                let (a, _) = self.abs(bus);
                self.pc = a;
                self.cycles += 3;
            }
            0x6C => {
                let addr = self.abs_addr(bus);
                // 6502 bug: JMP indirect wraps page
                let lo = bus.read(addr) as u16;
                let hi_addr = (addr & 0xFF00) | ((addr + 1) & 0x00FF);
                let hi = bus.read(hi_addr) as u16;
                self.pc = (hi << 8) | lo;
                self.cycles += 5;
            }

            // === JSR: Jump to Subroutine ===
            0x20 => {
                let addr = self.abs_addr(bus);
                let ret = self.pc.wrapping_sub(1);
                self.push(bus, (ret >> 8) as u8);
                self.push(bus, (ret & 0xFF) as u8);
                self.pc = addr;
                self.cycles += 6;
            }

            // === LDA: Load Accumulator ===
            0xA9 => {
                let v = self.imm(bus);
                self.a = v;
                self.set_nz(self.a);
                self.cycles += 2;
            }
            0xA5 => {
                let (a, _) = self.zp(bus);
                self.a = self.read_bus(bus, a);
                self.set_nz(self.a);
                self.cycles += 3;
            }
            0xB5 => {
                let (a, _) = self.zpx(bus);
                self.a = self.read_bus(bus, a);
                self.set_nz(self.a);
                self.cycles += 4;
            }
            0xAD => {
                let (a, _) = self.abs(bus);
                self.a = self.read_bus(bus, a);
                self.set_nz(self.a);
                self.cycles += 4;
            }
            0xBD => {
                let (a, p) = self.abx(bus);
                self.a = self.read_bus(bus, a);
                self.set_nz(self.a);
                self.cycles += 4 + p as u64;
            }
            0xB9 => {
                let (a, p) = self.aby(bus);
                self.a = self.read_bus(bus, a);
                self.set_nz(self.a);
                self.cycles += 4 + p as u64;
            }
            0xA1 => {
                let (a, _) = self.izx(bus);
                self.a = self.read_bus(bus, a);
                self.set_nz(self.a);
                self.cycles += 6;
            }
            0xB1 => {
                let (a, p) = self.izy(bus);
                self.a = self.read_bus(bus, a);
                self.set_nz(self.a);
                self.cycles += 5 + p as u64;
            }

            // === LDX: Load X ===
            0xA2 => {
                let v = self.imm(bus);
                self.x = v;
                self.set_nz(self.x);
                self.cycles += 2;
            }
            0xA6 => {
                let (a, _) = self.zp(bus);
                self.x = self.read_bus(bus, a);
                self.set_nz(self.x);
                self.cycles += 3;
            }
            0xB6 => {
                let (a, _) = self.zpy(bus);
                self.x = self.read_bus(bus, a);
                self.set_nz(self.x);
                self.cycles += 4;
            }
            0xAE => {
                let (a, _) = self.abs(bus);
                self.x = self.read_bus(bus, a);
                self.set_nz(self.x);
                self.cycles += 4;
            }
            0xBE => {
                let (a, p) = self.aby(bus);
                self.x = self.read_bus(bus, a);
                self.set_nz(self.x);
                self.cycles += 4 + p as u64;
            }

            // === LDY: Load Y ===
            0xA0 => {
                let v = self.imm(bus);
                self.y = v;
                self.set_nz(self.y);
                self.cycles += 2;
            }
            0xA4 => {
                let (a, _) = self.zp(bus);
                self.y = self.read_bus(bus, a);
                self.set_nz(self.y);
                self.cycles += 3;
            }
            0xB4 => {
                let (a, _) = self.zpx(bus);
                self.y = self.read_bus(bus, a);
                self.set_nz(self.y);
                self.cycles += 4;
            }
            0xAC => {
                let (a, _) = self.abs(bus);
                self.y = self.read_bus(bus, a);
                self.set_nz(self.y);
                self.cycles += 4;
            }
            0xBC => {
                let (a, p) = self.abx(bus);
                self.y = self.read_bus(bus, a);
                self.set_nz(self.y);
                self.cycles += 4 + p as u64;
            }

            // === LSR: Logical Shift Right ===
            0x4A => {
                self.lsr_acc();
                self.cycles += 2;
            }
            0x46 => {
                let (a, _) = self.zp(bus);
                self.lsr_mem(bus, a);
                self.cycles += 5;
            }
            0x56 => {
                let (a, _) = self.zpx(bus);
                self.lsr_mem(bus, a);
                self.cycles += 6;
            }
            0x4E => {
                let (a, _) = self.abs(bus);
                self.lsr_mem(bus, a);
                self.cycles += 6;
            }
            0x5E => {
                let (a, _) = self.abx(bus);
                self.lsr_mem(bus, a);
                self.cycles += 7;
            }

            // === NOP: No Operation ===
            0xEA => {
                self.cycles += 2;
            }

            // === ORA: Logical Inclusive OR ===
            0x09 => {
                let v = self.imm(bus);
                self.ora(v);
                self.cycles += 2;
            }
            0x05 => {
                let (a, _) = self.zp(bus);
                let v = self.read_bus(bus, a);
                self.ora(v);
                self.cycles += 3;
            }
            0x15 => {
                let (a, _) = self.zpx(bus);
                let v = self.read_bus(bus, a);
                self.ora(v);
                self.cycles += 4;
            }
            0x0D => {
                let (a, _) = self.abs(bus);
                let v = self.read_bus(bus, a);
                self.ora(v);
                self.cycles += 4;
            }
            0x1D => {
                let (a, p) = self.abx(bus);
                let v = self.read_bus(bus, a);
                self.ora(v);
                self.cycles += 4 + p as u64;
            }
            0x19 => {
                let (a, p) = self.aby(bus);
                let v = self.read_bus(bus, a);
                self.ora(v);
                self.cycles += 4 + p as u64;
            }
            0x01 => {
                let (a, _) = self.izx(bus);
                let v = self.read_bus(bus, a);
                self.ora(v);
                self.cycles += 6;
            }
            0x11 => {
                let (a, p) = self.izy(bus);
                let v = self.read_bus(bus, a);
                self.ora(v);
                self.cycles += 5 + p as u64;
            }

            // === PHA: Push Accumulator ===
            0x48 => {
                self.push(bus, self.a);
                self.cycles += 3;
            }

            // === PHP: Push Processor Status ===
            0x08 => {
                self.push(bus, self.ps | FLAG_B | FLAG_U);
                self.cycles += 3;
            }

            // === PLA: Pull Accumulator ===
            0x68 => {
                self.a = self.pop(bus);
                self.set_nz(self.a);
                self.cycles += 4;
            }

            // === PLP: Pull Processor Status ===
            0x28 => {
                self.ps = (self.pop(bus) & !FLAG_B) | FLAG_U;
                self.cycles += 4;
            }

            // === ROL: Rotate Left ===
            0x2A => {
                self.rol_acc();
                self.cycles += 2;
            }
            0x26 => {
                let (a, _) = self.zp(bus);
                self.rol_mem(bus, a);
                self.cycles += 5;
            }
            0x36 => {
                let (a, _) = self.zpx(bus);
                self.rol_mem(bus, a);
                self.cycles += 6;
            }
            0x2E => {
                let (a, _) = self.abs(bus);
                self.rol_mem(bus, a);
                self.cycles += 6;
            }
            0x3E => {
                let (a, _) = self.abx(bus);
                self.rol_mem(bus, a);
                self.cycles += 7;
            }

            // === ROR: Rotate Right ===
            0x6A => {
                self.ror_acc();
                self.cycles += 2;
            }
            0x66 => {
                let (a, _) = self.zp(bus);
                self.ror_mem(bus, a);
                self.cycles += 5;
            }
            0x76 => {
                let (a, _) = self.zpx(bus);
                self.ror_mem(bus, a);
                self.cycles += 6;
            }
            0x6E => {
                let (a, _) = self.abs(bus);
                self.ror_mem(bus, a);
                self.cycles += 6;
            }
            0x7E => {
                let (a, _) = self.abx(bus);
                self.ror_mem(bus, a);
                self.cycles += 7;
            }

            // === RTI: Return from Interrupt ===
            0x40 => {
                self.ps = (self.pop(bus) & !FLAG_B) | FLAG_U;
                let lo = self.pop(bus) as u16;
                let hi = self.pop(bus) as u16;
                self.pc = (hi << 8) | lo;
                self.cycles += 6;
            }

            // === RTS: Return from Subroutine ===
            0x60 => {
                let lo = self.pop(bus) as u16;
                let hi = self.pop(bus) as u16;
                self.pc = ((hi << 8) | lo).wrapping_add(1);
                self.cycles += 6;
            }

            // === SBC: Subtract with Carry ===
            0xE9 => {
                let v = self.imm(bus);
                self.sbc(v);
                self.cycles += 2;
            }
            0xE5 => {
                let (a, _) = self.zp(bus);
                let v = self.read_bus(bus, a);
                self.sbc(v);
                self.cycles += 3;
            }
            0xF5 => {
                let (a, _) = self.zpx(bus);
                let v = self.read_bus(bus, a);
                self.sbc(v);
                self.cycles += 4;
            }
            0xED => {
                let (a, _) = self.abs(bus);
                let v = self.read_bus(bus, a);
                self.sbc(v);
                self.cycles += 4;
            }
            0xFD => {
                let (a, p) = self.abx(bus);
                let v = self.read_bus(bus, a);
                self.sbc(v);
                self.cycles += 4 + p as u64;
            }
            0xF9 => {
                let (a, p) = self.aby(bus);
                let v = self.read_bus(bus, a);
                self.sbc(v);
                self.cycles += 4 + p as u64;
            }
            0xE1 => {
                let (a, _) = self.izx(bus);
                let v = self.read_bus(bus, a);
                self.sbc(v);
                self.cycles += 6;
            }
            0xF1 => {
                let (a, p) = self.izy(bus);
                let v = self.read_bus(bus, a);
                self.sbc(v);
                self.cycles += 5 + p as u64;
            }

            // === SEC: Set Carry ===
            0x38 => {
                self.ps |= FLAG_C;
                self.cycles += 2;
            }

            // === SED: Set Decimal ===
            0xF8 => {
                self.ps |= FLAG_D;
                self.cycles += 2;
            }

            // === SEI: Set Interrupt Disable ===
            0x78 => {
                self.ps |= FLAG_I;
                self.cycles += 2;
            }

            // === STA: Store Accumulator ===
            0x85 => {
                let (a, _) = self.zp(bus);
                bus.write(a, self.a);
                self.cycles += 3;
            }
            0x95 => {
                let (a, _) = self.zpx(bus);
                bus.write(a, self.a);
                self.cycles += 4;
            }
            0x8D => {
                let (a, _) = self.abs(bus);
                bus.write(a, self.a);
                self.cycles += 4;
            }
            0x9D => {
                let (a, _) = self.abx(bus);
                bus.write(a, self.a);
                self.cycles += 5;
            }
            0x99 => {
                let (a, _) = self.aby(bus);
                bus.write(a, self.a);
                self.cycles += 5;
            }
            0x81 => {
                let (a, _) = self.izx(bus);
                bus.write(a, self.a);
                self.cycles += 6;
            }
            0x91 => {
                let (a, _) = self.izy(bus);
                bus.write(a, self.a);
                self.cycles += 6;
            }

            // === STX: Store X ===
            0x86 => {
                let (a, _) = self.zp(bus);
                bus.write(a, self.x);
                self.cycles += 3;
            }
            0x96 => {
                let (a, _) = self.zpy(bus);
                bus.write(a, self.x);
                self.cycles += 4;
            }
            0x8E => {
                let (a, _) = self.abs(bus);
                bus.write(a, self.x);
                self.cycles += 4;
            }

            // === STY: Store Y ===
            0x84 => {
                let (a, _) = self.zp(bus);
                bus.write(a, self.y);
                self.cycles += 3;
            }
            0x94 => {
                let (a, _) = self.zpx(bus);
                bus.write(a, self.y);
                self.cycles += 4;
            }
            0x8C => {
                let (a, _) = self.abs(bus);
                bus.write(a, self.y);
                self.cycles += 4;
            }

            // === TAX: Transfer Accumulator to X ===
            0xAA => {
                self.x = self.a;
                self.set_nz(self.x);
                self.cycles += 2;
            }

            // === TAY: Transfer Accumulator to Y ===
            0xA8 => {
                self.y = self.a;
                self.set_nz(self.y);
                self.cycles += 2;
            }

            // === TSX: Transfer Stack Pointer to X ===
            0xBA => {
                self.x = self.sp;
                self.set_nz(self.x);
                self.cycles += 2;
            }

            // === TXA: Transfer X to Accumulator ===
            0x8A => {
                self.a = self.x;
                self.set_nz(self.a);
                self.cycles += 2;
            }

            // === TXS: Transfer X to Stack Pointer ===
            0x9A => {
                self.sp = self.x;
                self.cycles += 2;
            }

            // === TYA: Transfer Y to Accumulator ===
            0x98 => {
                self.a = self.y;
                self.set_nz(self.a);
                self.cycles += 2;
            }

            // Illegal opcodes - treated as NOPs with various sizes
            0x04 | 0x14 | 0x34 | 0x44 | 0x54 | 0x64 | 0x74 | 0x80 | 0x82 | 0x89 | 0xC2 | 0xD4
            | 0xE2 | 0xF4 => {
                // 2-byte NOP
                self.pc = self.pc.wrapping_add(1);
                self.cycles += 2;
            }
            0x0C => {
                // 3-byte NOP
                self.pc = self.pc.wrapping_add(2);
                self.cycles += 4;
            }
            0x1C | 0x3C | 0x5C | 0x7C | 0xDC | 0xFC => {
                // 3-byte NOP with possible extra cycle
                self.pc = self.pc.wrapping_add(2);
                self.cycles += 4;
            }
            0x1A | 0x3A | 0x5A | 0x7A | 0xDA | 0xFA => {
                // 1-byte NOP
                self.cycles += 2;
            }

            // All other illegal opcodes: 1-byte NOP
            _ => {
                self.cycles += 2;
            }
        }

        self.cycles - start_cycles
    }

    // === Addressing Modes ===
    // Returns (address, page_crossed)

    /// Immediate: returns value directly
    #[inline]
    fn imm(&mut self, bus: &impl CpuBus) -> u8 {
        let v = bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        v
    }

    /// Zero Page
    #[inline]
    fn zp(&mut self, bus: &impl CpuBus) -> (u16, bool) {
        let addr = bus.read(self.pc) as u16;
        self.pc = self.pc.wrapping_add(1);
        (addr, false)
    }

    /// Zero Page, X
    #[inline]
    fn zpx(&mut self, bus: &impl CpuBus) -> (u16, bool) {
        let addr = bus.read(self.pc).wrapping_add(self.x) as u16;
        self.pc = self.pc.wrapping_add(1);
        (addr, false)
    }

    /// Zero Page, Y
    #[inline]
    fn zpy(&mut self, bus: &impl CpuBus) -> (u16, bool) {
        let addr = bus.read(self.pc).wrapping_add(self.y) as u16;
        self.pc = self.pc.wrapping_add(1);
        (addr, false)
    }

    /// Absolute address helper
    #[inline]
    fn abs_addr(&mut self, bus: &impl CpuBus) -> u16 {
        let lo = bus.read(self.pc) as u16;
        let hi = bus.read(self.pc.wrapping_add(1)) as u16;
        self.pc = self.pc.wrapping_add(2);
        (hi << 8) | lo
    }

    /// Absolute
    #[inline]
    fn abs(&mut self, bus: &impl CpuBus) -> (u16, bool) {
        (self.abs_addr(bus), false)
    }

    /// Absolute, X
    #[inline]
    fn abx(&mut self, bus: &impl CpuBus) -> (u16, bool) {
        let base = self.abs_addr(bus);
        let addr = base.wrapping_add(self.x as u16);
        let page_crossed = (base & 0xFF00) != (addr & 0xFF00);
        (addr, page_crossed)
    }

    /// Absolute, Y
    #[inline]
    fn aby(&mut self, bus: &impl CpuBus) -> (u16, bool) {
        let base = self.abs_addr(bus);
        let addr = base.wrapping_add(self.y as u16);
        let page_crossed = (base & 0xFF00) != (addr & 0xFF00);
        (addr, page_crossed)
    }

    /// (Indirect, X) - zero page indirect indexed by X
    #[inline]
    fn izx(&mut self, bus: &impl CpuBus) -> (u16, bool) {
        let zp_addr = bus.read(self.pc).wrapping_add(self.x) as u16;
        self.pc = self.pc.wrapping_add(1);
        let lo = bus.read(zp_addr) as u16;
        let hi = bus.read(zp_addr.wrapping_add(1) & 0xFF) as u16;
        ((hi << 8) | lo, false)
    }

    /// (Indirect), Y - zero page indirect indexed by Y
    #[inline]
    fn izy(&mut self, bus: &impl CpuBus) -> (u16, bool) {
        let zp_addr = bus.read(self.pc) as u16;
        self.pc = self.pc.wrapping_add(1);
        let lo = bus.read(zp_addr) as u16;
        let hi = bus.read(zp_addr.wrapping_add(1) & 0xFF) as u16;
        let base = (hi << 8) | lo;
        let addr = base.wrapping_add(self.y as u16);
        let page_crossed = (base & 0xFF00) != (addr & 0xFF00);
        (addr, page_crossed)
    }

    /// Read from bus (handles IO region)
    #[inline]
    fn read_bus(&self, bus: &impl CpuBus, addr: u16) -> u8 {
        bus.read(addr)
    }

    // === ALU Operations ===

    /// ADC: Add with Carry
    fn adc(&mut self, operand: u8) {
        if self.ps & FLAG_D != 0 {
            // BCD mode
            let mut lo = (self.a & 0x0F) + (operand & 0x0F) + (self.ps & FLAG_C);
            if lo > 9 {
                lo += 6;
            }
            let mut hi = (self.a >> 4) + (operand >> 4) + if lo > 0x0F { 1 } else { 0 };
            if hi > 9 {
                hi += 6;
            }
            let result = (lo & 0x0F) | (hi << 4);
            let overflow = (!((self.a ^ operand) & 0x80) & ((self.a ^ (hi << 4)) & 0x80)) != 0;
            self.ps = (self.ps & !(FLAG_N | FLAG_V | FLAG_Z | FLAG_C))
                | if hi > 0x0F { FLAG_C } else { 0 }
                | if result == 0 { FLAG_Z } else { 0 }
                | if result & 0x80 != 0 { FLAG_N } else { 0 }
                | if overflow { FLAG_V } else { 0 };
            self.a = result;
        } else {
            let carry = self.ps & FLAG_C;
            let sum = self.a as u16 + operand as u16 + carry as u16;
            let result = sum as u8;
            let overflow = (!(self.a ^ operand) & (self.a ^ result) & 0x80) != 0;
            self.ps = (self.ps & !(FLAG_N | FLAG_V | FLAG_Z | FLAG_C))
                | if sum > 0xFF { FLAG_C } else { 0 }
                | if result == 0 { FLAG_Z } else { 0 }
                | if result & 0x80 != 0 { FLAG_N } else { 0 }
                | if overflow { FLAG_V } else { 0 };
            self.a = result;
        }
    }

    /// SBC: Subtract with Carry
    fn sbc(&mut self, operand: u8) {
        if self.ps & FLAG_D != 0 {
            // BCD mode
            let carry = self.ps & FLAG_C;
            let mut lo = (self.a & 0x0F)
                .wrapping_sub(operand & 0x0F)
                .wrapping_sub(1 - carry);
            if lo & 0x10 != 0 {
                lo = lo.wrapping_sub(6);
            }
            let mut hi = (self.a >> 4)
                .wrapping_sub(operand >> 4)
                .wrapping_sub(if lo & 0x10 != 0 { 1 } else { 0 });
            if hi & 0x10 != 0 {
                hi = hi.wrapping_sub(6);
            }
            let result = (lo & 0x0F) | (hi << 4);
            let raw = self.a as i16 - operand as i16 - (1 - carry) as i16;
            let overflow = ((self.a ^ operand) & (self.a ^ result) & 0x80) != 0;
            self.ps = (self.ps & !(FLAG_N | FLAG_V | FLAG_Z | FLAG_C))
                | if raw >= 0 { FLAG_C } else { 0 }
                | if result == 0 { FLAG_Z } else { 0 }
                | if result & 0x80 != 0 { FLAG_N } else { 0 }
                | if overflow { FLAG_V } else { 0 };
            self.a = result;
        } else {
            let carry = self.ps & FLAG_C;
            let diff = self.a as i16 - operand as i16 - (1 - carry) as i16;
            let result = diff as u8;
            let overflow = ((self.a ^ operand) & (self.a ^ result) & 0x80) != 0;
            self.ps = (self.ps & !(FLAG_N | FLAG_V | FLAG_Z | FLAG_C))
                | if diff >= 0 { FLAG_C } else { 0 }
                | if result == 0 { FLAG_Z } else { 0 }
                | if result & 0x80 != 0 { FLAG_N } else { 0 }
                | if overflow { FLAG_V } else { 0 };
            self.a = result;
        }
    }

    /// AND
    #[inline]
    fn and(&mut self, operand: u8) {
        self.a &= operand;
        self.set_nz(self.a);
    }

    /// ORA
    #[inline]
    fn ora(&mut self, operand: u8) {
        self.a |= operand;
        self.set_nz(self.a);
    }

    /// EOR
    #[inline]
    fn eor(&mut self, operand: u8) {
        self.a ^= operand;
        self.set_nz(self.a);
    }

    /// Compare
    #[inline]
    fn cmp(&mut self, reg: u8, operand: u8) {
        let diff = reg.wrapping_sub(operand);
        self.ps = (self.ps & !(FLAG_N | FLAG_Z | FLAG_C))
            | if diff & 0x80 != 0 { FLAG_N } else { 0 }
            | if diff == 0 { FLAG_Z } else { 0 }
            | if reg >= operand { FLAG_C } else { 0 };
    }

    /// BIT test
    fn bit(&mut self, bus: &impl CpuBus, addr: u16) {
        let value = bus.read(addr);
        self.ps = (self.ps & !(FLAG_N | FLAG_V | FLAG_Z))
            | (value & 0xC0)  // N and V come from the operand
            | if self.a & value == 0 { FLAG_Z } else { 0 };
    }

    /// ASL accumulator
    fn asl_acc(&mut self) {
        self.ps = (self.ps & !FLAG_C) | if self.a & 0x80 != 0 { FLAG_C } else { 0 };
        self.a <<= 1;
        self.set_nz(self.a);
    }

    /// ASL memory
    fn asl_mem(&mut self, bus: &mut impl CpuBus, addr: u16) {
        let value = bus.read(addr);
        self.ps = (self.ps & !FLAG_C) | if value & 0x80 != 0 { FLAG_C } else { 0 };
        let result = value << 1;
        bus.write(addr, result);
        self.set_nz(result);
    }

    /// LSR accumulator
    fn lsr_acc(&mut self) {
        self.ps = (self.ps & !FLAG_C) | if self.a & 0x01 != 0 { FLAG_C } else { 0 };
        self.a >>= 1;
        self.set_nz(self.a);
    }

    /// LSR memory
    fn lsr_mem(&mut self, bus: &mut impl CpuBus, addr: u16) {
        let value = bus.read(addr);
        self.ps = (self.ps & !FLAG_C) | if value & 0x01 != 0 { FLAG_C } else { 0 };
        let result = value >> 1;
        bus.write(addr, result);
        self.set_nz(result);
    }

    /// ROL accumulator
    fn rol_acc(&mut self) {
        let old_carry = self.ps & FLAG_C;
        self.ps = (self.ps & !FLAG_C) | if self.a & 0x80 != 0 { FLAG_C } else { 0 };
        self.a = (self.a << 1) | old_carry;
        self.set_nz(self.a);
    }

    /// ROL memory
    fn rol_mem(&mut self, bus: &mut impl CpuBus, addr: u16) {
        let value = bus.read(addr);
        let old_carry = self.ps & FLAG_C;
        self.ps = (self.ps & !FLAG_C) | if value & 0x80 != 0 { FLAG_C } else { 0 };
        let result = (value << 1) | old_carry;
        bus.write(addr, result);
        self.set_nz(result);
    }

    /// ROR accumulator
    fn ror_acc(&mut self) {
        let old_carry = self.ps & FLAG_C;
        self.ps = (self.ps & !FLAG_C) | if self.a & 0x01 != 0 { FLAG_C } else { 0 };
        self.a = (self.a >> 1) | (old_carry << 7);
        self.set_nz(self.a);
    }

    /// ROR memory
    fn ror_mem(&mut self, bus: &mut impl CpuBus, addr: u16) {
        let value = bus.read(addr);
        let old_carry = self.ps & FLAG_C;
        self.ps = (self.ps & !FLAG_C) | if value & 0x01 != 0 { FLAG_C } else { 0 };
        let result = (value >> 1) | (old_carry << 7);
        bus.write(addr, result);
        self.set_nz(result);
    }

    /// DEC memory
    fn dec(&mut self, bus: &mut impl CpuBus, addr: u16) {
        let value = bus.read(addr).wrapping_sub(1);
        bus.write(addr, value);
        self.set_nz(value);
    }

    /// INC memory
    fn inc(&mut self, bus: &mut impl CpuBus, addr: u16) {
        let value = bus.read(addr).wrapping_add(1);
        bus.write(addr, value);
        self.set_nz(value);
    }

    /// Branch helper: returns extra cycles (0, 1, or 2)
    fn branch(&mut self, bus: &impl CpuBus, condition: bool) -> u64 {
        let offset = bus.read(self.pc) as i8;
        self.pc = self.pc.wrapping_add(1);
        if condition {
            let old_pc = self.pc;
            self.pc = self.pc.wrapping_add(offset as u16);
            // Extra cycle for page crossing
            if (old_pc & 0xFF00) != (self.pc & 0xFF00) {
                2
            } else {
                1
            }
        } else {
            0
        }
    }
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

/// Bus interface for CPU memory access
pub trait CpuBus {
    /// Read a byte from the given address
    fn read(&self, addr: u16) -> u8;
    /// Write a byte to the given address
    fn write(&mut self, addr: u16, value: u8);
    /// Read a 16-bit word (little-endian)
    fn read_u16(&self, addr: u16) -> u16 {
        let lo = self.read(addr) as u16;
        let hi = self.read(addr.wrapping_add(1)) as u16;
        (hi << 8) | lo
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestBus {
        ram: [u8; 0x10000],
    }

    impl TestBus {
        fn new() -> Self {
            Self { ram: [0; 0x10000] }
        }
    }

    impl CpuBus for TestBus {
        fn read(&self, addr: u16) -> u8 {
            self.ram[addr as usize]
        }
        fn write(&mut self, addr: u16, value: u8) {
            self.ram[addr as usize] = value;
        }
    }

    #[test]
    fn test_reset() {
        let mut cpu = Cpu::new();
        let mut bus = TestBus::new();
        bus.ram[0xFFFC] = 0x00;
        bus.ram[0xFFFD] = 0x80;
        cpu.reset(bus.read_u16(RESET_VECTOR));
        assert_eq!(cpu.pc, 0x8000);
        assert_eq!(cpu.ps, FLAG_I | FLAG_U);
    }

    #[test]
    fn test_lda_immediate() {
        let mut cpu = Cpu::new();
        let mut bus = TestBus::new();
        cpu.pc = 0x0200;
        bus.ram[0x0200] = 0xA9; // LDA immediate
        bus.ram[0x0201] = 0x42;
        cpu.step(&mut bus);
        assert_eq!(cpu.a, 0x42);
        assert!(!cpu.flag_n());
        assert!(!cpu.flag_z());
    }

    #[test]
    fn test_lda_zero_flag() {
        let mut cpu = Cpu::new();
        let mut bus = TestBus::new();
        cpu.pc = 0x0200;
        bus.ram[0x0200] = 0xA9; // LDA immediate
        bus.ram[0x0201] = 0x00;
        cpu.step(&mut bus);
        assert_eq!(cpu.a, 0x00);
        assert!(cpu.flag_z());
    }

    #[test]
    fn test_adc_basic() {
        let mut cpu = Cpu::new();
        let mut bus = TestBus::new();
        cpu.pc = 0x0200;
        cpu.a = 0x10;
        cpu.ps &= !FLAG_C;
        bus.ram[0x0200] = 0x69; // ADC immediate
        bus.ram[0x0201] = 0x20;
        cpu.step(&mut bus);
        assert_eq!(cpu.a, 0x30);
        assert!(!cpu.flag_c());
    }

    #[test]
    fn test_adc_carry() {
        let mut cpu = Cpu::new();
        let mut bus = TestBus::new();
        cpu.pc = 0x0200;
        cpu.a = 0xFF;
        cpu.ps &= !FLAG_C;
        bus.ram[0x0200] = 0x69; // ADC immediate
        bus.ram[0x0201] = 0x01;
        cpu.step(&mut bus);
        assert_eq!(cpu.a, 0x00);
        assert!(cpu.flag_c());
        assert!(cpu.flag_z());
    }

    #[test]
    fn test_sbc_basic() {
        let mut cpu = Cpu::new();
        let mut bus = TestBus::new();
        cpu.pc = 0x0200;
        cpu.a = 0x50;
        cpu.ps |= FLAG_C; // Set carry for SBC
        bus.ram[0x0200] = 0xE9; // SBC immediate
        bus.ram[0x0201] = 0x30;
        cpu.step(&mut bus);
        assert_eq!(cpu.a, 0x20);
        assert!(cpu.flag_c());
    }

    #[test]
    fn test_branch_taken() {
        let mut cpu = Cpu::new();
        let mut bus = TestBus::new();
        cpu.pc = 0x0200;
        cpu.ps &= !FLAG_Z;
        bus.ram[0x0200] = 0xD0; // BNE
        bus.ram[0x0201] = 0x10; // offset +16
        cpu.step(&mut bus);
        assert_eq!(cpu.pc, 0x0212);
    }

    #[test]
    fn test_branch_not_taken() {
        let mut cpu = Cpu::new();
        let mut bus = TestBus::new();
        cpu.pc = 0x0200;
        cpu.ps |= FLAG_Z;
        bus.ram[0x0200] = 0xD0; // BNE
        bus.ram[0x0201] = 0x10;
        cpu.step(&mut bus);
        assert_eq!(cpu.pc, 0x0202);
    }

    #[test]
    fn test_jsr_rts() {
        let mut cpu = Cpu::new();
        let mut bus = TestBus::new();
        cpu.pc = 0x0200;
        cpu.sp = 0xFF;
        bus.ram[0x0200] = 0x20; // JSR
        bus.ram[0x0201] = 0x00;
        bus.ram[0x0202] = 0x03; // -> 0x0300
        bus.ram[0x0300] = 0x60; // RTS
        cpu.step(&mut bus);
        assert_eq!(cpu.pc, 0x0300);
        cpu.step(&mut bus);
        assert_eq!(cpu.pc, 0x0203);
    }
}
