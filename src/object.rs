use std::error::Error as StdError;
use std::fmt;

use hashbrown::HashMap;
use iced_x86::{Decoder, Instruction, MemorySize, Mnemonic};
use object::{Object, ObjectSection};

use crate::graph::Graph;

const TEXT_SECTION_NAME: &str = ".text";

const ALIGN_SEQUENCES: &[&[u8]] = &[
    &[0xCC, 0xCC],
    &[0x0F, 0x1F, 0x00],
    &[0x0F, 0x1F, 0x40, 0x00],
    &[0x0F, 0x1F, 0x44, 0x00, 0x00],
    &[0x0F, 0x1F, 0x80, 0x00, 0x00, 0x00, 0x00],
    &[0x0F, 0x1F, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00],
];

/// Represents the text section of an object file.
#[derive(Debug)]
pub struct ObjectCode<'file, 'data> {
    text: object::Section<'file, 'data>,
    entry: u64,
}

impl<'file, 'data> ObjectCode<'file, 'data> {
    /// Loads code from an object file.
    pub fn load(file: &'file object::read::File<'data>) -> Result<Self, Error> {
        let text = file
            .section_by_name(TEXT_SECTION_NAME)
            .ok_or(Error::MissingTextSection)?;

        Ok(Self {
            entry: file.entry(),
            text,
        })
    }

    /// Returns the relative address of the entrypoint in the text section.
    pub fn entrypoint(&self) -> u64 {
        self.entry - self.text_section_base()
    }

    /// Returns the base address of the text section.
    pub fn text_section_base(&self) -> u64 {
        self.text.address()
    }
}

/// Metadata for code extracted from an object file.
#[derive(Debug, Default)]
pub struct CodeMetadata {
    pub(crate) call_graph: Graph<u64>,
    pub(crate) functions: HashMap<u64, FunctionMetadata>,
}

impl CodeMetadata {
    /// Loads an object file using the provided path.
    pub fn load(obj: &ObjectCode<'_, '_>, seeds: impl IntoIterator<Item = u64>) -> Result<Self, Error> {
        let slice = obj.text.data().map_err(|err| Error::Other(err.into()))?;
        let mut object = Self::default();
        object.load_func(obj.entrypoint(), slice);
        for seed in seeds {
            object.load_func(seed, slice);
        }
        Ok(object)
    }

    fn load_func(&mut self, addr: u64, segment: &[u8]) {
        let mut instruction = Instruction::default();
        let mut work = vec![addr];

        while let Some(addr) = work.pop() {
            if self.call_graph.has_vertex(addr) {
                continue;
            }

            let addr_usize = addr as usize;
            let len = segment[addr_usize..]
                .windows(16)
                .position(is_endp)
                .unwrap_or(segment.len() - addr_usize);

            let body = &segment[addr_usize..addr_usize + len];
            self.functions.insert(addr, FunctionMetadata::from_slice(body));

            let mut decoder = Decoder::new(64, body, 0);

            while decoder.can_decode() {
                decoder.decode_out(&mut instruction);

                match instruction.mnemonic() {
                    Mnemonic::Call | Mnemonic::Jmp => {
                        let rel_addr = instruction.memory_displacement64();
                        let next_addr = if instruction.memory_size() == MemorySize::QwordOffset {
                            addr + rel_addr
                        } else {
                            let rel_addr = rel_addr as i64;
                            if rel_addr.is_negative() && rel_addr.unsigned_abs() > addr {
                                continue;
                            }
                            addr.checked_add_signed(rel_addr).unwrap()
                        };

                        if !(addr..addr + len as u64).contains(&next_addr) {
                            self.call_graph.add_edge(addr, next_addr);
                            if (0..segment.len() as u64).contains(&next_addr) {
                                work.push(next_addr);
                            } else {
                                self.functions.insert(next_addr, FunctionMetadata::default());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// Returns the call graph with relative addresses as vertices.
    #[inline]
    pub fn call_graph(&self) -> &Graph<u64> {
        &self.call_graph
    }

    /// Returns the function metadata for the given relative address.
    #[inline]
    pub(crate) fn get_function(&self, addr: u64) -> Option<&FunctionMetadata> {
        self.functions.get(&addr)
    }
}

#[derive(Debug)]
pub enum Error {
    MissingTextSection,
    Other(Box<dyn StdError>),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::MissingTextSection => write!(f, "missing .text section"),
            Error::Other(err) => write!(f, "{}", err),
        }
    }
}

impl StdError for Error {}

#[derive(Debug, Default, Clone)]
pub(crate) struct FunctionMetadata {
    opcodes: Vec<Mnemonic>,
}

impl FunctionMetadata {
    #[inline]
    pub fn new(opcodes: Vec<Mnemonic>) -> Self {
        Self { opcodes }
    }

    #[inline]
    pub fn opcodes(&self) -> &[Mnemonic] {
        &self.opcodes
    }

    pub fn from_slice(slice: &[u8]) -> Self {
        let mut opcodes = vec![];
        let mut decoder = Decoder::new(64, slice, 0);
        let mut instruction = Instruction::default();

        while decoder.can_decode() {
            decoder.decode_out(&mut instruction);
            opcodes.push(instruction.mnemonic());
        }

        Self::new(opcodes)
    }
}

fn is_endp(slice: &[u8]) -> bool {
    match slice {
        // call followed by alignment bytes
        [0xE8, _, _, _, _, rem @ ..] => ALIGN_SEQUENCES.iter().any(|seq| rem.starts_with(seq)),
        // return followed by alignment bytes
        [0xC3, rem @ ..] => ALIGN_SEQUENCES.iter().any(|seq| rem.starts_with(seq)),
        _ => false,
    }
}
