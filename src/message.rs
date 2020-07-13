use crate::{Blob, BlobTag, IO};
use core::convert::TryInto;
use core::mem::{size_of, transmute};
use storage_endian::{BEu16, BEu32};

values!(pub MessageVersion(u8) {
    CURRENT = 0x00,
});

values!(pub MessageType(u8) {
    HELLO           = 0x00,
    STATUS          = 0x01,
    DATA            = 0x02,
    PING            = 0x03,
    LOOKUP          = 0x04,
    INVOKE          = 0x05,
    ADD_OBJECT      = 0x06,
    REMOVE_OBJECT   = 0x07,
    SUBSCRIBE       = 0x08,
    UNSUBSCRIBE     = 0x09,
    NOTIFY          = 0x10,
    MONITOR         = 0x11,
});

values!(pub MessageAttr(u32) {
    UNSPEC      = 0x00,
    STATUS      = 0x01,
    OBJPATH     = 0x02,
    OBJID       = 0x03,
    METHOD      = 0x04,
    OBJTYPE     = 0x05,
    SIGNATURE   = 0x06,
    DATA        = 0x07,
    TARGET      = 0x08,
    ACTIVE      = 0x09,
    NO_REPLY    = 0x0a,
    SUBSCRIBERS = 0x0b,
    USER        = 0x0c,
    GROUP       = 0x0d,
});

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct MessageHeader {
    pub version: MessageVersion,
    pub message: MessageType,
    pub sequence: BEu16,
    pub peer: BEu32,
}

impl MessageHeader {
    pub const SIZE: usize = size_of::<Self>();

    /// Create MessageHeader from a byte array
    pub fn from_bytes(buffer: [u8; Self::SIZE]) -> Self {
        unsafe { transmute(buffer) }
    }
    // Dump out bytes of MessageHeader
    pub fn to_bytes(self) -> [u8; Self::SIZE] {
        unsafe { core::mem::transmute(self) }
    }
}

#[derive(Copy, Clone)]
pub struct Message<'a> {
    pub header: MessageHeader,
    pub blob: Blob<'a>,
}

impl<'a> Message<'a> {
    pub fn from_io<T: IO>(io: &mut T, buffer: &'a mut [u8]) -> Result<Self, T::Error> {
        let (pre_buffer, buffer) = buffer.split_at_mut(MessageHeader::SIZE + BlobTag::SIZE);

        // Read in the message header and the following blob tag
        io.get(pre_buffer)?;

        let (header, tag) = pre_buffer.split_at(MessageHeader::SIZE);

        let header = MessageHeader::from_bytes(header.try_into().unwrap());
        assert_eq!(header.version, MessageVersion::CURRENT);

        let tag = BlobTag::from_bytes(tag.try_into().unwrap());
        assert!(tag.is_valid());

        // Get a slice the size of the blob's data bytes (do we need to worry about padding here?)
        let data = &mut buffer[..tag.inner_len()];

        // Receive data into slice
        io.get(data)?;

        // Create the blob from our parts
        let blob = Blob::from_tag_and_data(tag, data).unwrap();

        Ok(Message { header, blob })
    }
}

impl core::fmt::Debug for Message<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(
            f,
            "Message({:?} seq={} peer={:08x}, size={})",
            self.header.message,
            self.header.sequence,
            self.header.peer,
            self.blob.data.len()
        )
    }
}

pub struct MessageBuilder<'a> {
    buffer: &'a mut [u8],
    offset: usize,
}

impl<'a> MessageBuilder<'a> {
    pub fn new(buffer: &'a mut [u8], header: MessageHeader) -> Result<Self, ()> {
        if buffer.len() < MessageHeader::SIZE + BlobTag::SIZE {
            return Err(());
        }

        let header_buf = &mut buffer[..MessageHeader::SIZE];
        let header_buf: &mut [u8; MessageHeader::SIZE] = header_buf.try_into().unwrap();
        *header_buf = header.to_bytes();

        let offset = MessageHeader::SIZE + BlobTag::SIZE;

        Ok(Self { buffer, offset })
    }

    pub fn finish(self) -> &'a [u8] {
        // Update tag with correct size
        let tag = BlobTag::new(0, self.offset - MessageHeader::SIZE).unwrap();
        let tag_buf = &mut self.buffer[MessageHeader::SIZE..MessageHeader::SIZE + BlobTag::SIZE];
        let tag_buf: &mut [u8; BlobTag::SIZE] = tag_buf.try_into().unwrap();
        *tag_buf = tag.to_bytes();

        &self.buffer[..self.offset]
    }
}
impl<'a> Into<&'a [u8]> for MessageBuilder<'a> {
    fn into(self) -> &'a [u8] {
        self.finish()
    }
}
