//! TI variable data.

pub trait Payload {
    const FILE_EXTENSION: &'static str;
}

pub struct Real {}

/// Options for how the program should treat perceived errors in reading raw data.
#[derive(Clone, Copy)]
pub struct ReadMode(u32);

impl ReadMode {
    /// Error mode specifies that a function should fail when it encounters malformed data.
    pub fn error() -> Self {
        ReadMode(0)
    }

    /// Fix mode specifies that a function should try its best to rectify malformed data and
    /// proceed if possible.
    pub fn fix() -> Self {
        ReadMode(1)
    }

    /// Ignore mode specifies that a function should proceed with creating an invalid representation
    /// when malformed data is encountered.
    pub unsafe fn ignore() -> Self {
        ReadMode(2)
    }
}

impl Default for ReadMode {
    fn default() -> Self {
        ReadMode::error()
    }
}

#[derive(Clone, Default)]
pub struct VariableReadOptions;

pub struct Variable<T: Payload> {
    payload: T,
}

impl<T: Payload> Variable<T> {
    pub fn bytes(&self) -> Vec<u8> {
        todo!()
    }
}

/// Error-handling options for reading in a file.
/// 
/// See [`ReadMode`] for the different modes.
#[derive(Clone, Default)]
pub struct FileReadOptions {
    pub signature: ReadMode,
    pub variable_length: ReadMode,
    pub variable: VariableReadOptions,
    pub checksum: ReadMode,
}

const FILE_SIGNATURE_SIZE: usize = 0x0B;
const FILE_COMMENT_SIZE: usize = 0x2A;
const FILE_VARIABLE_LENGTH_SIZE: usize = 0x02;
const FILE_CHECKSUM_SIZE: usize = 0x02;

/// Data representation of a file exported from a TI calculator.
///
/// Files are wrappers around variables and other data, and they include metadata
/// such as a signature, which identifies a file as TI-compatible, and an
/// optional comment.
pub struct File<T: Payload> {
    signature: [u8; FILE_SIGNATURE_SIZE],
    comment: [u8; FILE_COMMENT_SIZE],
    variable_length: [u8; FILE_VARIABLE_LENGTH_SIZE],
    variable: Variable<T>,
    checksum: [u8; FILE_CHECKSUM_SIZE],
}

impl<T: Payload> File<T> {
    pub const SIGNATURE_OFFSET: usize = 0x00;
    pub const SIGNATURE_SIZE: usize = FILE_SIGNATURE_SIZE;
    pub const COMMENT_OFFSET: usize = Self::SIGNATURE_OFFSET + Self::SIGNATURE_SIZE;
    pub const COMMENT_SIZE: usize = FILE_COMMENT_SIZE;
    pub const VARIABLE_LENGTH_OFFSET: usize = Self::COMMENT_OFFSET + Self::COMMENT_SIZE;
    pub const VARIABLE_LENGTH_SIZE: usize = FILE_VARIABLE_LENGTH_SIZE;
    pub const HEADER_SIZE: usize =
        Self::SIGNATURE_SIZE + Self::COMMENT_SIZE + Self::VARIABLE_LENGTH_SIZE;
    pub const VARIABLE_OFFSET: usize = Self::VARIABLE_LENGTH_OFFSET + Self::VARIABLE_LENGTH_SIZE;
    pub const CHECKSUM_SIZE: usize = FILE_CHECKSUM_SIZE;

    /// Gets the size in bytes of the file.
    ///
    /// The file size should equal the size of the header ([`File::HEADER_SIZE`]) plus
    /// the size of the variable ([`File::variable_length`]) plus the size of this checksum
    /// [`File::CHECKSUM_SIZE`].
    pub fn size(&self) -> usize {
        Self::SIGNATURE_SIZE
            + Self::COMMENT_SIZE
            + Self::VARIABLE_LENGTH_SIZE
            + self.variable_length() as usize
            + Self::CHECKSUM_SIZE
    }

    /// Gets the raw representation of the entire file in memory, including
    /// the header data and checksum.
    ///
    /// See [`File::variable`] for the variable stored in the file.
    pub fn bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.size());
        bytes.extend(self.signature.iter());
        bytes.extend(self.comment.iter());
        bytes.extend(self.variable_length.iter());
        bytes.extend(self.variable.bytes().iter());
        bytes.extend(self.checksum.iter());
        bytes
    }

    /// Gets the "signature," which identifies the data as usable on TI devices.
    ///
    /// This is always the string `**TI83F*` followed by the bytes `0x1A`, `0x0A`, and `0x00`.
    pub fn signature(&self) -> &[u8; FILE_SIGNATURE_SIZE] {
        &self.signature
    }

    /// Gets the "signature," which identifies the data as usable on TI devices,
    /// for mutation.
    ///
    /// # Safety
    ///
    /// Calculator-generated data has an 11-byte signature consisting of `**TI83F*` followed by
    /// the bytes `0x1A`, `0x0A`, and `0x00`. Changing the signature to anything else risks
    /// making the data unusable.
    pub unsafe fn signature_mut(&mut self) -> &mut [u8; FILE_SIGNATURE_SIZE] {
        &mut self.signature
    }

    fn comment_null_terminator_position(&self) -> Option<usize> {
        self.comment.iter().position(|c| *c == 0)
    }

    fn comment_ending_spaces(&self) -> usize {
        const SPACE_AS_NUMBER: u8 = ' ' as u8;
        self.comment
            .iter()
            .rev()
            .take_while(|c| **c == SPACE_AS_NUMBER)
            .count()
    }

    /// Gets the region of data reserved for a comment and parses it as a UTF-8 string.
    ///
    /// This data is left empty when generated on the calculator, but other programs
    /// which have modified this region may not necessarily have formatted it in UTF-8.
    ///
    /// Internally, the string is either zero-terminated or padded with space characters.
    /// When the `trim` parameter is set to `true`, the space padding on the right will
    /// be removed from the result.
    ///
    /// Use [`File::comment_raw`] to extract the bytes.
    pub fn comment(&self, mut trim: bool) -> Result<String, std::string::FromUtf8Error> {
        let comment = match self.comment_null_terminator_position() {
            Some(pos) => {
                trim = false;
                &self.comment[..pos]
            }
            None => &self.comment,
        };
        let mut comment = String::from_utf8(comment.to_vec())?;
        if trim {
            let trimmed_len = comment.trim_end_matches(' ').len();
            comment.truncate(trimmed_len);
        }
        Ok(comment)
    }

    /// Gets the size in bytes of the comment in the region of data reserved for it.
    ///
    /// Comments can be zero-terminated or padded to the right with spaces. This function
    /// ignores both when calculating the length.
    pub fn comment_length(&self) -> usize {
        match self.comment_null_terminator_position() {
            Some(null_char_position) => null_char_position,
            None => Self::COMMENT_SIZE - self.comment_ending_spaces(),
        }
    }

    /// Gets whether the comment data is zero-terminated (`true`) or padded (`false`).
    ///
    /// If the comment fills the entire space in memory (see [`File::COMMENT_SIZE`]),
    /// this returns `false`.
    pub fn is_comment_zero_terminated(&self) -> bool {
        self.comment.contains(&0)
    }

    /// Forces the comment region of data to be zero-terminated.
    ///
    /// The comment region can end by being padded with spaces, and if this is the case,
    /// this changes the string to be zero-terminated.
    pub fn make_comment_zero_terminated(&mut self) {
        if self.is_comment_zero_terminated() {
            return;
        }
        let ending_spaces = self.comment_ending_spaces();
        if ending_spaces == 0 {
            // There is no room at the end for a null terminator.
            return;
        }
        let first_space_index = Self::COMMENT_SIZE - ending_spaces;
        self.comment[first_space_index] = 0;
    }

    /// Forces the comment region of data to end with space-character padding.
    ///
    /// The comment region can be zero-terminated, and if this is the case, this replaces
    /// the termination with right-padding made of space characters.
    pub fn make_comment_padded(&mut self) {
        let Some(pad_start) = self.comment_null_terminator_position() else { return };
        const SPACE_AS_NUMBER: u8 = ' ' as u8;
        for i in pad_start..Self::COMMENT_SIZE {
            self.comment[i] = SPACE_AS_NUMBER;
        }
    }

    /// Stores a UTF-8 string in the region of data reserved for a comment.
    ///
    /// If `zero_terminated` is set to `false`, the comment will be padded at the end
    /// with spaces.
    ///
    /// This function will only take as many bytes from the string as will fit
    /// in the data region; see [`File::COMMENT_SIZE`].
    pub fn set_comment(&mut self, comment: &str, zero_terminated: bool) {
        let mut bytes = comment.bytes();

        for i in 0..Self::COMMENT_SIZE {
            match bytes.next() {
                Some(b) => self.comment[i] = b,
                None => {
                    if zero_terminated {
                        self.comment[i] = 0;
                        return;
                    }
                    const SPACE_AS_NUMBER: u8 = ' ' as u8;
                    self.comment[i] = SPACE_AS_NUMBER;
                }
            }
        }
    }

    /// Gets the raw data from the region reserved for a comment.
    ///
    /// The comment is either zero-terminated or padded with spaces.
    pub fn comment_raw(&self) -> &[u8; FILE_COMMENT_SIZE] {
        &self.comment
    }

    /// Gets the raw data from the region reserved for a comment for mutation.
    ///
    /// Changing the comment is safe; the calculator never reads it.
    pub fn comment_raw_mut(&mut self) -> &mut [u8; FILE_COMMENT_SIZE] {
        &mut self.comment
    }

    /// Gets the size in bytes of the variable region of data.
    pub fn variable_length(&self) -> u16 {
        u16::from_le_bytes(self.variable_length)
    }

    /// Gets the region of data that holds the size in bytes of the variable region
    /// of data.
    ///
    /// The length is stored as a little-endian integer.
    pub fn variable_length_raw(&self) -> &[u8; FILE_VARIABLE_LENGTH_SIZE] {
        &self.variable_length
    }

    /// Gets the bytes that represent the size in bytes of the variable region of data
    /// as raw mutable data.
    ///
    /// # Safety
    ///
    /// The bytes must represent a little-endian integer that matches the length
    /// in bytes of the variable section of the data.
    pub unsafe fn variable_length_raw_mut(&mut self) -> &mut [u8; FILE_VARIABLE_LENGTH_SIZE] {
        &mut self.variable_length
    }

    /// Gets the variable stored in the file.
    pub fn variable(&self) -> &Variable<T> {
        &self.variable
    }

    /// Gets the variable stored in the file for mutation.
    pub fn variable_mut(&mut self) -> &mut Variable<T> {
        &mut self.variable
    }

    /// Gets how many bytes from the start the checksum data is offset.
    pub fn checksum_offset(&self) -> usize {
        Self::VARIABLE_OFFSET + self.variable_length() as usize
    }

    /// Gets the checksum at the end of the file.
    ///
    /// This is equal to the lower 16 bits of the sum of the bytes in the variable section
    /// of the data.
    pub fn checksum(&self) -> u16 {
        u16::from_le_bytes(self.checksum)
    }

    /// Gets the bytes representing the checksum at the end of the file as mutable data.
    ///
    /// # Safety
    ///
    /// The checksum equals the lower 16 bits of the sum of the bytes in the variable section
    /// of the data. The data is stored as a little-endian integer.
    pub fn checksum_raw(&self) -> &[u8; FILE_CHECKSUM_SIZE] {
        &self.checksum
    }

    /// Gets the bytes representing the checksum at the end of the file as mutable data.
    ///
    /// This must equal the lower 16 bits of the sum of the bytes in the variable section
    /// of the data. The data is stored as a little-endian integer.
    pub unsafe fn checksum_raw_mut(&mut self) -> &mut [u8; FILE_CHECKSUM_SIZE] {
        &mut self.checksum
    }
}
