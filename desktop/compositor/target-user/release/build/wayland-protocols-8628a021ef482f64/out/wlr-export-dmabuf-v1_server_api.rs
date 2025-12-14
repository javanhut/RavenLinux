use std::os::raw::{c_char, c_void};
const NULLPTR: *const c_void = 0 as *const c_void;
static mut types_null: [*const sys::common::wl_interface; 10] = [
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
];
#[doc = "manager to inform clients and begin capturing\n\nThis object is a manager with which to start capturing from sources."]
pub mod zwlr_export_dmabuf_manager_v1 {
    use super::sys::common::{wl_argument, wl_array, wl_interface, wl_message};
    use super::sys::server::*;
    use super::{
        smallvec, types_null, AnonymousObject, Argument, ArgumentType, Interface, Main, Message,
        MessageDesc, MessageGroup, Object, ObjectMetadata, Resource, NULLPTR,
    };
    use std::os::raw::c_char;
    #[derive(Debug)]
    #[non_exhaustive]
    pub enum Request {
        #[doc = "capture a frame from an output\n\nCapture the next frame of an entire output."]
        CaptureOutput {
            frame: Main<super::zwlr_export_dmabuf_frame_v1::ZwlrExportDmabufFrameV1>,
            overlay_cursor: i32,
            output: super::wl_output::WlOutput,
        },
        #[doc = "destroy the manager\n\nAll objects created by the manager will still remain valid, until their\nappropriate destroy request has been called.\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "capture_output",
                since: 1,
                signature: &[
                    super::ArgumentType::NewId,
                    super::ArgumentType::Int,
                    super::ArgumentType::Object,
                ],
                destructor: false,
            },
            super::MessageDesc {
                name: "destroy",
                since: 1,
                signature: &[],
                destructor: true,
            },
        ];
        type Map = super::ResourceMap;
        fn is_destructor(&self) -> bool {
            match *self {
                Request::Destroy => true,
                _ => false,
            }
        }
        fn opcode(&self) -> u16 {
            match *self {
                Request::CaptureOutput { .. } => 0,
                Request::Destroy => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::CaptureOutput { .. } => 1,
                Request::Destroy => 1,
            }
        }
        fn child<Meta: ObjectMetadata>(
            opcode: u16,
            version: u32,
            meta: &Meta,
        ) -> Option<Object<Meta>> {
            match opcode {
                0 => Some(Object::from_interface::<
                    super::zwlr_export_dmabuf_frame_v1::ZwlrExportDmabufFrameV1,
                >(version, meta.child())),
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            match msg.opcode {
                0 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::CaptureOutput {
                        frame: {
                            if let Some(Argument::NewId(val)) = args.next() {
                                map.get_new(val).ok_or(())?
                            } else {
                                return Err(());
                            }
                        },
                        overlay_cursor: {
                            if let Some(Argument::Int(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        output: {
                            if let Some(Argument::Object(val)) = args.next() {
                                map.get(val).ok_or(())?.into()
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                1 => Ok(Request::Destroy),
                _ => Err(()),
            }
        }
        fn into_raw(self, sender_id: u32) -> Message {
            panic!("Request::into_raw can not be used Server-side.")
        }
        unsafe fn from_raw_c(
            obj: *mut ::std::os::raw::c_void,
            opcode: u32,
            args: *const wl_argument,
        ) -> Result<Request, ()> {
            match opcode {
                0 => {
                    let _args = ::std::slice::from_raw_parts(args, 3);
                    Ok(Request::CaptureOutput {
                        frame: {
                            let me =
                                Resource::<ZwlrExportDmabufManagerV1>::from_c_ptr(obj as *mut _);
                            me . make_child_for :: < super :: zwlr_export_dmabuf_frame_v1 :: ZwlrExportDmabufFrameV1 > (_args [0] . n) . unwrap ()
                        },
                        overlay_cursor: _args[1].i,
                        output: Resource::<super::wl_output::WlOutput>::from_c_ptr(
                            _args[2].o as *mut _,
                        )
                        .into(),
                    })
                }
                1 => Ok(Request::Destroy),
                _ => return Err(()),
            }
        }
        fn as_raw_c_in<F, T>(self, f: F) -> T
        where
            F: FnOnce(u32, &mut [wl_argument]) -> T,
        {
            panic!("Request::as_raw_c_in can not be used Server-side.")
        }
    }
    #[derive(Debug)]
    #[non_exhaustive]
    pub enum Event {}
    impl super::MessageGroup for Event {
        const MESSAGES: &'static [super::MessageDesc] = &[];
        type Map = super::ResourceMap;
        fn is_destructor(&self) -> bool {
            match *self {}
        }
        fn opcode(&self) -> u16 {
            match *self {}
        }
        fn since(&self) -> u32 {
            match *self {}
        }
        fn child<Meta: ObjectMetadata>(
            opcode: u16,
            version: u32,
            meta: &Meta,
        ) -> Option<Object<Meta>> {
            match opcode {
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            panic!("Event::from_raw can not be used Server-side.")
        }
        fn into_raw(self, sender_id: u32) -> Message {
            match self {}
        }
        unsafe fn from_raw_c(
            obj: *mut ::std::os::raw::c_void,
            opcode: u32,
            args: *const wl_argument,
        ) -> Result<Event, ()> {
            panic!("Event::from_raw_c can not be used Server-side.")
        }
        fn as_raw_c_in<F, T>(self, f: F) -> T
        where
            F: FnOnce(u32, &mut [wl_argument]) -> T,
        {
            match self {}
        }
    }
    #[derive(Clone, Eq, PartialEq)]
    pub struct ZwlrExportDmabufManagerV1(Resource<ZwlrExportDmabufManagerV1>);
    impl AsRef<Resource<ZwlrExportDmabufManagerV1>> for ZwlrExportDmabufManagerV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwlrExportDmabufManagerV1>> for ZwlrExportDmabufManagerV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwlrExportDmabufManagerV1(value)
        }
    }
    impl From<ZwlrExportDmabufManagerV1> for Resource<ZwlrExportDmabufManagerV1> {
        #[inline]
        fn from(value: ZwlrExportDmabufManagerV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwlrExportDmabufManagerV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwlrExportDmabufManagerV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwlr_export_dmabuf_manager_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwlr_export_dmabuf_manager_v1_interface }
        }
    }
    impl ZwlrExportDmabufManagerV1 {}
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_CAPTURE_OUTPUT_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    static mut zwlr_export_dmabuf_manager_v1_requests_capture_output_types: [*const wl_interface;
        3] = [
        unsafe {
            &super::zwlr_export_dmabuf_frame_v1::zwlr_export_dmabuf_frame_v1_interface
                as *const wl_interface
        },
        NULLPTR as *const wl_interface,
        unsafe { &super::wl_output::wl_output_interface as *const wl_interface },
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwlr_export_dmabuf_manager_v1_requests: [wl_message; 2] = [
        wl_message {
            name: b"capture_output\0" as *const u8 as *const c_char,
            signature: b"nio\0" as *const u8 as *const c_char,
            types: unsafe {
                &zwlr_export_dmabuf_manager_v1_requests_capture_output_types as *const _
            },
        },
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwlr_export_dmabuf_manager_v1_interface: wl_interface = wl_interface {
        name: b"zwlr_export_dmabuf_manager_v1\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 2,
        requests: unsafe { &zwlr_export_dmabuf_manager_v1_requests as *const _ },
        event_count: 0,
        events: NULLPTR as *const wl_message,
    };
}
#[doc = "a DMA-BUF frame\n\nThis object represents a single DMA-BUF frame.\n\nIf the capture is successful, the compositor will first send a \"frame\"\nevent, followed by one or several \"object\". When the frame is available\nfor readout, the \"ready\" event is sent.\n\nIf the capture failed, the \"cancel\" event is sent. This can happen anytime\nbefore the \"ready\" event.\n\nOnce either a \"ready\" or a \"cancel\" event is received, the client should\ndestroy the frame. Once an \"object\" event is received, the client is\nresponsible for closing the associated file descriptor.\n\nAll frames are read-only and may not be written into or altered."]
pub mod zwlr_export_dmabuf_frame_v1 {
    use super::sys::common::{wl_argument, wl_array, wl_interface, wl_message};
    use super::sys::server::*;
    use super::{
        smallvec, types_null, AnonymousObject, Argument, ArgumentType, Interface, Main, Message,
        MessageDesc, MessageGroup, Object, ObjectMetadata, Resource, NULLPTR,
    };
    use std::os::raw::c_char;
    #[doc = "frame flags\n\nSpecial flags that should be respected by the client."]
    #[repr(u32)]
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[non_exhaustive]
    pub enum Flags {
        #[doc = "clients should copy frame before processing"]
        Transient = 1,
    }
    impl Flags {
        pub fn from_raw(n: u32) -> Option<Flags> {
            match n {
                1 => Some(Flags::Transient),
                _ => Option::None,
            }
        }
        pub fn to_raw(&self) -> u32 {
            *self as u32
        }
    }
    #[doc = "cancel reason\n\nIndicates reason for cancelling the frame."]
    #[repr(u32)]
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[non_exhaustive]
    pub enum CancelReason {
        #[doc = "temporary error, source will produce more frames"]
        Temporary = 0,
        #[doc = "fatal error, source will not produce frames"]
        Permanent = 1,
        #[doc = "temporary error, source will produce more frames"]
        Resizing = 2,
    }
    impl CancelReason {
        pub fn from_raw(n: u32) -> Option<CancelReason> {
            match n {
                0 => Some(CancelReason::Temporary),
                1 => Some(CancelReason::Permanent),
                2 => Some(CancelReason::Resizing),
                _ => Option::None,
            }
        }
        pub fn to_raw(&self) -> u32 {
            *self as u32
        }
    }
    #[derive(Debug)]
    #[non_exhaustive]
    pub enum Request {
        #[doc = "delete this object, used or not\n\nUnreferences the frame. This request must be called as soon as its no\nlonger used.\n\nIt can be called at any time by the client. The client will still have\nto close any FDs it has been given.\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[super::MessageDesc {
            name: "destroy",
            since: 1,
            signature: &[],
            destructor: true,
        }];
        type Map = super::ResourceMap;
        fn is_destructor(&self) -> bool {
            match *self {
                Request::Destroy => true,
            }
        }
        fn opcode(&self) -> u16 {
            match *self {
                Request::Destroy => 0,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::Destroy => 1,
            }
        }
        fn child<Meta: ObjectMetadata>(
            opcode: u16,
            version: u32,
            meta: &Meta,
        ) -> Option<Object<Meta>> {
            match opcode {
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            match msg.opcode {
                0 => Ok(Request::Destroy),
                _ => Err(()),
            }
        }
        fn into_raw(self, sender_id: u32) -> Message {
            panic!("Request::into_raw can not be used Server-side.")
        }
        unsafe fn from_raw_c(
            obj: *mut ::std::os::raw::c_void,
            opcode: u32,
            args: *const wl_argument,
        ) -> Result<Request, ()> {
            match opcode {
                0 => Ok(Request::Destroy),
                _ => return Err(()),
            }
        }
        fn as_raw_c_in<F, T>(self, f: F) -> T
        where
            F: FnOnce(u32, &mut [wl_argument]) -> T,
        {
            panic!("Request::as_raw_c_in can not be used Server-side.")
        }
    }
    #[derive(Debug)]
    #[non_exhaustive]
    pub enum Event {
        #[doc = "a frame description\n\nMain event supplying the client with information about the frame. If the\ncapture didn't fail, this event is always emitted first before any other\nevents.\n\nThis event is followed by a number of \"object\" as specified by the\n\"num_objects\" argument."]
        Frame {
            width: u32,
            height: u32,
            offset_x: u32,
            offset_y: u32,
            buffer_flags: u32,
            flags: Flags,
            format: u32,
            mod_high: u32,
            mod_low: u32,
            num_objects: u32,
        },
        #[doc = "an object description\n\nEvent which serves to supply the client with the file descriptors\ncontaining the data for each object.\n\nAfter receiving this event, the client must always close the file\ndescriptor as soon as they're done with it and even if the frame fails."]
        Object {
            index: u32,
            fd: ::std::os::unix::io::RawFd,
            size: u32,
            offset: u32,
            stride: u32,
            plane_index: u32,
        },
        #[doc = "indicates frame is available for reading\n\nThis event is sent as soon as the frame is presented, indicating it is\navailable for reading. This event includes the time at which\npresentation happened at.\n\nThe timestamp is expressed as tv_sec_hi, tv_sec_lo, tv_nsec triples,\neach component being an unsigned 32-bit value. Whole seconds are in\ntv_sec which is a 64-bit value combined from tv_sec_hi and tv_sec_lo,\nand the additional fractional part in tv_nsec as nanoseconds. Hence,\nfor valid timestamps tv_nsec must be in [0, 999999999]. The seconds part\nmay have an arbitrary offset at start.\n\nAfter receiving this event, the client should destroy this object."]
        Ready {
            tv_sec_hi: u32,
            tv_sec_lo: u32,
            tv_nsec: u32,
        },
        #[doc = "indicates the frame is no longer valid\n\nIf the capture failed or if the frame is no longer valid after the\n\"frame\" event has been emitted, this event will be used to inform the\nclient to scrap the frame.\n\nIf the failure is temporary, the client may capture again the same\nsource. If the failure is permanent, any further attempts to capture the\nsame source will fail again.\n\nAfter receiving this event, the client should destroy this object."]
        Cancel { reason: CancelReason },
    }
    impl super::MessageGroup for Event {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "frame",
                since: 1,
                signature: &[
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                ],
                destructor: false,
            },
            super::MessageDesc {
                name: "object",
                since: 1,
                signature: &[
                    super::ArgumentType::Uint,
                    super::ArgumentType::Fd,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                ],
                destructor: false,
            },
            super::MessageDesc {
                name: "ready",
                since: 1,
                signature: &[
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                ],
                destructor: false,
            },
            super::MessageDesc {
                name: "cancel",
                since: 1,
                signature: &[super::ArgumentType::Uint],
                destructor: false,
            },
        ];
        type Map = super::ResourceMap;
        fn is_destructor(&self) -> bool {
            match *self {
                _ => false,
            }
        }
        fn opcode(&self) -> u16 {
            match *self {
                Event::Frame { .. } => 0,
                Event::Object { .. } => 1,
                Event::Ready { .. } => 2,
                Event::Cancel { .. } => 3,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Event::Frame { .. } => 1,
                Event::Object { .. } => 1,
                Event::Ready { .. } => 1,
                Event::Cancel { .. } => 1,
            }
        }
        fn child<Meta: ObjectMetadata>(
            opcode: u16,
            version: u32,
            meta: &Meta,
        ) -> Option<Object<Meta>> {
            match opcode {
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            panic!("Event::from_raw can not be used Server-side.")
        }
        fn into_raw(self, sender_id: u32) -> Message {
            match self {
                Event::Frame {
                    width,
                    height,
                    offset_x,
                    offset_y,
                    buffer_flags,
                    flags,
                    format,
                    mod_high,
                    mod_low,
                    num_objects,
                } => Message {
                    sender_id: sender_id,
                    opcode: 0,
                    args: smallvec![
                        Argument::Uint(width),
                        Argument::Uint(height),
                        Argument::Uint(offset_x),
                        Argument::Uint(offset_y),
                        Argument::Uint(buffer_flags),
                        Argument::Uint(flags.to_raw()),
                        Argument::Uint(format),
                        Argument::Uint(mod_high),
                        Argument::Uint(mod_low),
                        Argument::Uint(num_objects),
                    ],
                },
                Event::Object {
                    index,
                    fd,
                    size,
                    offset,
                    stride,
                    plane_index,
                } => Message {
                    sender_id: sender_id,
                    opcode: 1,
                    args: smallvec![
                        Argument::Uint(index),
                        Argument::Fd(fd),
                        Argument::Uint(size),
                        Argument::Uint(offset),
                        Argument::Uint(stride),
                        Argument::Uint(plane_index),
                    ],
                },
                Event::Ready {
                    tv_sec_hi,
                    tv_sec_lo,
                    tv_nsec,
                } => Message {
                    sender_id: sender_id,
                    opcode: 2,
                    args: smallvec![
                        Argument::Uint(tv_sec_hi),
                        Argument::Uint(tv_sec_lo),
                        Argument::Uint(tv_nsec),
                    ],
                },
                Event::Cancel { reason } => Message {
                    sender_id: sender_id,
                    opcode: 3,
                    args: smallvec![Argument::Uint(reason.to_raw()),],
                },
            }
        }
        unsafe fn from_raw_c(
            obj: *mut ::std::os::raw::c_void,
            opcode: u32,
            args: *const wl_argument,
        ) -> Result<Event, ()> {
            panic!("Event::from_raw_c can not be used Server-side.")
        }
        fn as_raw_c_in<F, T>(self, f: F) -> T
        where
            F: FnOnce(u32, &mut [wl_argument]) -> T,
        {
            match self {
                Event::Frame {
                    width,
                    height,
                    offset_x,
                    offset_y,
                    buffer_flags,
                    flags,
                    format,
                    mod_high,
                    mod_low,
                    num_objects,
                } => {
                    let mut _args_array: [wl_argument; 10] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = width;
                    _args_array[1].u = height;
                    _args_array[2].u = offset_x;
                    _args_array[3].u = offset_y;
                    _args_array[4].u = buffer_flags;
                    _args_array[5].u = flags.to_raw();
                    _args_array[6].u = format;
                    _args_array[7].u = mod_high;
                    _args_array[8].u = mod_low;
                    _args_array[9].u = num_objects;
                    f(0, &mut _args_array)
                }
                Event::Object {
                    index,
                    fd,
                    size,
                    offset,
                    stride,
                    plane_index,
                } => {
                    let mut _args_array: [wl_argument; 6] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = index;
                    _args_array[1].h = fd;
                    _args_array[2].u = size;
                    _args_array[3].u = offset;
                    _args_array[4].u = stride;
                    _args_array[5].u = plane_index;
                    f(1, &mut _args_array)
                }
                Event::Ready {
                    tv_sec_hi,
                    tv_sec_lo,
                    tv_nsec,
                } => {
                    let mut _args_array: [wl_argument; 3] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = tv_sec_hi;
                    _args_array[1].u = tv_sec_lo;
                    _args_array[2].u = tv_nsec;
                    f(2, &mut _args_array)
                }
                Event::Cancel { reason } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = reason.to_raw();
                    f(3, &mut _args_array)
                }
            }
        }
    }
    #[derive(Clone, Eq, PartialEq)]
    pub struct ZwlrExportDmabufFrameV1(Resource<ZwlrExportDmabufFrameV1>);
    impl AsRef<Resource<ZwlrExportDmabufFrameV1>> for ZwlrExportDmabufFrameV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwlrExportDmabufFrameV1>> for ZwlrExportDmabufFrameV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwlrExportDmabufFrameV1(value)
        }
    }
    impl From<ZwlrExportDmabufFrameV1> for Resource<ZwlrExportDmabufFrameV1> {
        #[inline]
        fn from(value: ZwlrExportDmabufFrameV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwlrExportDmabufFrameV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwlrExportDmabufFrameV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwlr_export_dmabuf_frame_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwlr_export_dmabuf_frame_v1_interface }
        }
    }
    impl ZwlrExportDmabufFrameV1 {
        #[doc = "a frame description\n\nMain event supplying the client with information about the frame. If the\ncapture didn't fail, this event is always emitted first before any other\nevents.\n\nThis event is followed by a number of \"object\" as specified by the\n\"num_objects\" argument."]
        pub fn frame(
            &self,
            width: u32,
            height: u32,
            offset_x: u32,
            offset_y: u32,
            buffer_flags: u32,
            flags: Flags,
            format: u32,
            mod_high: u32,
            mod_low: u32,
            num_objects: u32,
        ) -> () {
            let msg = Event::Frame {
                width: width,
                height: height,
                offset_x: offset_x,
                offset_y: offset_y,
                buffer_flags: buffer_flags,
                flags: flags,
                format: format,
                mod_high: mod_high,
                mod_low: mod_low,
                num_objects: num_objects,
            };
            self.0.send(msg);
        }
        #[doc = "an object description\n\nEvent which serves to supply the client with the file descriptors\ncontaining the data for each object.\n\nAfter receiving this event, the client must always close the file\ndescriptor as soon as they're done with it and even if the frame fails."]
        pub fn object(
            &self,
            index: u32,
            fd: ::std::os::unix::io::RawFd,
            size: u32,
            offset: u32,
            stride: u32,
            plane_index: u32,
        ) -> () {
            let msg = Event::Object {
                index: index,
                fd: fd,
                size: size,
                offset: offset,
                stride: stride,
                plane_index: plane_index,
            };
            self.0.send(msg);
        }
        #[doc = "indicates frame is available for reading\n\nThis event is sent as soon as the frame is presented, indicating it is\navailable for reading. This event includes the time at which\npresentation happened at.\n\nThe timestamp is expressed as tv_sec_hi, tv_sec_lo, tv_nsec triples,\neach component being an unsigned 32-bit value. Whole seconds are in\ntv_sec which is a 64-bit value combined from tv_sec_hi and tv_sec_lo,\nand the additional fractional part in tv_nsec as nanoseconds. Hence,\nfor valid timestamps tv_nsec must be in [0, 999999999]. The seconds part\nmay have an arbitrary offset at start.\n\nAfter receiving this event, the client should destroy this object."]
        pub fn ready(&self, tv_sec_hi: u32, tv_sec_lo: u32, tv_nsec: u32) -> () {
            let msg = Event::Ready {
                tv_sec_hi: tv_sec_hi,
                tv_sec_lo: tv_sec_lo,
                tv_nsec: tv_nsec,
            };
            self.0.send(msg);
        }
        #[doc = "indicates the frame is no longer valid\n\nIf the capture failed or if the frame is no longer valid after the\n\"frame\" event has been emitted, this event will be used to inform the\nclient to scrap the frame.\n\nIf the failure is temporary, the client may capture again the same\nsource. If the failure is permanent, any further attempts to capture the\nsame source will fail again.\n\nAfter receiving this event, the client should destroy this object."]
        pub fn cancel(&self, reason: CancelReason) -> () {
            let msg = Event::Cancel { reason: reason };
            self.0.send(msg);
        }
    }
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_FRAME_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_OBJECT_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_READY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_CANCEL_SINCE: u32 = 1u32;
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwlr_export_dmabuf_frame_v1_requests: [wl_message; 1] = [wl_message {
        name: b"destroy\0" as *const u8 as *const c_char,
        signature: b"\0" as *const u8 as *const c_char,
        types: unsafe { &types_null as *const _ },
    }];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwlr_export_dmabuf_frame_v1_events: [wl_message; 4] = [
        wl_message {
            name: b"frame\0" as *const u8 as *const c_char,
            signature: b"uuuuuuuuuu\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"object\0" as *const u8 as *const c_char,
            signature: b"uhuuuu\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"ready\0" as *const u8 as *const c_char,
            signature: b"uuu\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"cancel\0" as *const u8 as *const c_char,
            signature: b"u\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwlr_export_dmabuf_frame_v1_interface: wl_interface = wl_interface {
        name: b"zwlr_export_dmabuf_frame_v1\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 1,
        requests: unsafe { &zwlr_export_dmabuf_frame_v1_requests as *const _ },
        event_count: 4,
        events: unsafe { &zwlr_export_dmabuf_frame_v1_events as *const _ },
    };
}
