use std::error::Error as StdError;
use std::path::Path;
use std::{fmt, fs, io};

use hashbrown::HashMap;
use iced_x86::{Decoder, Instruction, MemorySize, Mnemonic};
use object::{Object, ObjectSection};

use crate::graph::Graph;

const TEXT_SECTION_NAME: &str = ".text";
const FUNCTION_EPILOGUES: &[&[u8]] = &[&[0x0F, 0x1F], &[0x0F, 0x0B], &[0xCC, 0xCC]];

/// Metadata for an object file which contains executable code.
#[derive(Debug, Default)]
pub struct ObjectMetadata {
    call_graph: Graph<u64>,
    functions: HashMap<u64, FunctionMetadata>,
    entry: u64,
    segment_base: u64,
}

impl ObjectMetadata {
    pub(crate) fn new(
        call_graph: Graph<u64>,
        functions: HashMap<u64, FunctionMetadata>,
        entry: u64,
        segment_base: u64,
    ) -> Self {
        Self {
            call_graph,
            functions,
            entry,
            segment_base,
        }
    }

    /// Loads an object file using the provided path.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Error> {
        let bytes = fs::read(path.as_ref()).map_err(Error::Read)?;
        let obj = object::read::File::parse(&bytes[..]).map_err(|err| Error::Other(err.into()))?;
        let text = obj
            .section_by_name(TEXT_SECTION_NAME)
            .ok_or(Error::MissingTextSection)?;
        let slice = text.data().map_err(|err| Error::Other(err.into()))?;

        let entry = obj.entry() - text.address();
        let mut object = Self {
            entry,
            segment_base: text.address(),
            ..Default::default()
        };
        object.load_func(entry, slice);
        Ok(object)
    }

    fn load_func(&mut self, addr: u64, segment: &[u8]) {
        if self.call_graph.has_vertex(addr) {
            return;
        }

        let addr_usize = addr as usize;
        let len = segment[addr_usize..]
            .windows(2)
            .position(|bytes| FUNCTION_EPILOGUES.contains(&bytes))
            .unwrap_or(segment.len() - addr_usize);

        let body = &segment[addr_usize..addr_usize + len];
        self.functions.insert(addr, FunctionMetadata::from_slice(body));

        let mut decoder = Decoder::new(64, body, 0);
        let mut instruction = Instruction::default();

        while decoder.can_decode() {
            decoder.decode_out(&mut instruction);

            match instruction.mnemonic() {
                Mnemonic::Call | Mnemonic::Jmp => {
                    let rel_addr = instruction.memory_displacement64();
                    let next_addr = match instruction.memory_size() {
                        MemorySize::QwordOffset => addr + rel_addr,
                        _ => addr.checked_add_signed(rel_addr as i64).unwrap(),
                    };
                    if !(addr..addr + len as u64).contains(&next_addr) {
                        self.call_graph.add_edge(addr, next_addr);
                        if (0..segment.len() as u64).contains(&next_addr) {
                            self.load_func(next_addr, segment);
                        } else {
                            self.functions.insert(next_addr, FunctionMetadata::default());
                        }
                    }
                }
                _ => {}
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

    /// Returns a relative address for the entry point.
    #[inline]
    pub fn entry(&self) -> u64 {
        self.entry
    }

    /// returns the base address of the text segment.
    #[inline]
    pub(crate) fn text_segment_base(&self) -> u64 {
        self.segment_base
    }
}

#[derive(Debug)]
pub enum Error {
    Read(io::Error),
    MissingTextSection,
    Other(Box<dyn StdError>),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Read(err) => write!(f, "failed to read object file: {}", err),
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
