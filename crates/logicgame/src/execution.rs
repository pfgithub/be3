use uuid::Uuid;

pub type MemoryAddress = usize;
pub type StorageId = usize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Instruction {
    Call {
        component: Uuid,
        inputs: Vec<MemoryAddress>,
        outputs: Vec<MemoryAddress>,
    },
    Not {
        input: MemoryAddress,
        output: MemoryAddress,
    },
    ReadStorage {
        storage: StorageId,
        output: MemoryAddress,
    },
    SaveStorage {
        storage: StorageId,
        input: MemoryAddress,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Vm {
    pub memory: Vec<u64>,
    pub storage: Vec<u64>,
    pub instructions: Vec<Instruction>,
}

impl Vm {
    pub fn execute(&mut self) {
        for instruction in &self.instructions {
            match instruction {
                Instruction::Call { component, .. } => {
                    panic!("calling component {component} is not implemented")
                }
                Instruction::Not { input, output } => {
                    self.memory[*output] = !self.memory[*input];
                }
                Instruction::ReadStorage { storage, output } => {
                    self.memory[*output] = self.storage[*storage];
                }
                Instruction::SaveStorage { storage, input } => {
                    self.storage[*storage] = self.memory[*input];
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_reads_and_writes_memory() {
        let mut vm = Vm {
            memory: vec![0x00ff, 0],
            instructions: vec![Instruction::Not {
                input: 0,
                output: 1,
            }],
            ..Vm::default()
        };

        vm.execute();

        assert_eq!(vm.memory, vec![0x00ff, !0x00ff]);
    }

    #[test]
    fn storage_can_be_saved_and_read() {
        let mut vm = Vm {
            memory: vec![42, 0],
            storage: vec![0],
            instructions: vec![
                Instruction::SaveStorage {
                    storage: 0,
                    input: 0,
                },
                Instruction::ReadStorage {
                    storage: 0,
                    output: 1,
                },
            ],
        };

        vm.execute();

        assert_eq!(vm.storage, vec![42]);
        assert_eq!(vm.memory, vec![42, 42]);
    }

    #[test]
    fn instructions_execute_in_order() {
        let mut vm = Vm {
            memory: vec![0, 0],
            storage: vec![7],
            instructions: vec![
                Instruction::ReadStorage {
                    storage: 0,
                    output: 0,
                },
                Instruction::Not {
                    input: 0,
                    output: 1,
                },
                Instruction::SaveStorage {
                    storage: 0,
                    input: 1,
                },
            ],
        };

        vm.execute();

        assert_eq!(vm.storage[0], !7);
    }

    #[test]
    #[should_panic(expected = "is not implemented")]
    fn call_is_not_implemented() {
        let mut vm = Vm {
            instructions: vec![Instruction::Call {
                component: Uuid::nil(),
                inputs: vec![],
                outputs: vec![],
            }],
            ..Vm::default()
        };

        vm.execute();
    }
}
