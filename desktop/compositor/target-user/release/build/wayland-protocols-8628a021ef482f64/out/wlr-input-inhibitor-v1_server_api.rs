use std::os::raw::{c_char, c_void};
const NULLPTR: *const c_void = 0 as *const c_void;
static mut types_null: [*const sys::common::wl_interface; 0] = [];
#[doc = "inhibits input events to other clients\n\nClients can use this interface to prevent input events from being sent to\nany surfaces but its own, which is useful for example in lock screen\nsoftware. It is assumed that access to this interface will be locked down\nto whitelisted clients by the compositor."]
pub mod zwlr_input_inhibit_manager_v1 {
    use super::sys::common::{wl_argument, wl_array, wl_interface, wl_message};
    use super::sys::server::*;
    use super::{
        smallvec, types_null, AnonymousObject, Argument, ArgumentType, Interface, Main, Message,
        MessageDesc, MessageGroup, Object, ObjectMetadata, Resource, NULLPTR,
    };
    use std::os::raw::c_char;
    #[repr(u32)]
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[non_exhaustive]
    pub enum Error {
        #[doc = "an input inhibitor is already in use on the compositor"]
        AlreadyInhibited = 0,
    }
    impl Error {
        pub fn from_raw(n: u32) -> Option<Error> {
            match n {
                0 => Some(Error::AlreadyInhibited),
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
        #[doc = "inhibit input to other clients\n\nActivates the input inhibitor. As long as the inhibitor is active, the\ncompositor will not send input events to other clients."]
        GetInhibitor {
            id: Main<super::zwlr_input_inhibitor_v1::ZwlrInputInhibitorV1>,
        },
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[super::MessageDesc {
            name: "get_inhibitor",
            since: 1,
            signature: &[super::ArgumentType::NewId],
            destructor: false,
        }];
        type Map = super::ResourceMap;
        fn is_destructor(&self) -> bool {
            match *self {
                _ => false,
            }
        }
        fn opcode(&self) -> u16 {
            match *self {
                Request::GetInhibitor { .. } => 0,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::GetInhibitor { .. } => 1,
            }
        }
        fn child<Meta: ObjectMetadata>(
            opcode: u16,
            version: u32,
            meta: &Meta,
        ) -> Option<Object<Meta>> {
            match opcode {
                0 => Some(Object::from_interface::<
                    super::zwlr_input_inhibitor_v1::ZwlrInputInhibitorV1,
                >(version, meta.child())),
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            match msg.opcode {
                0 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::GetInhibitor {
                        id: {
                            if let Some(Argument::NewId(val)) = args.next() {
                                map.get_new(val).ok_or(())?
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
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
                    let _args = ::std::slice::from_raw_parts(args, 1);
                    Ok(Request::GetInhibitor {
                        id: {
                            let me =
                                Resource::<ZwlrInputInhibitManagerV1>::from_c_ptr(obj as *mut _);
                            me . make_child_for :: < super :: zwlr_input_inhibitor_v1 :: ZwlrInputInhibitorV1 > (_args [0] . n) . unwrap ()
                        },
                    })
                }
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
    pub struct ZwlrInputInhibitManagerV1(Resource<ZwlrInputInhibitManagerV1>);
    impl AsRef<Resource<ZwlrInputInhibitManagerV1>> for ZwlrInputInhibitManagerV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwlrInputInhibitManagerV1>> for ZwlrInputInhibitManagerV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwlrInputInhibitManagerV1(value)
        }
    }
    impl From<ZwlrInputInhibitManagerV1> for Resource<ZwlrInputInhibitManagerV1> {
        #[inline]
        fn from(value: ZwlrInputInhibitManagerV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwlrInputInhibitManagerV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwlrInputInhibitManagerV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwlr_input_inhibit_manager_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwlr_input_inhibit_manager_v1_interface }
        }
    }
    impl ZwlrInputInhibitManagerV1 {}
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_GET_INHIBITOR_SINCE: u32 = 1u32;
    static mut zwlr_input_inhibit_manager_v1_requests_get_inhibitor_types: [*const wl_interface;
        1] = [unsafe {
        &super::zwlr_input_inhibitor_v1::zwlr_input_inhibitor_v1_interface as *const wl_interface
    }];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwlr_input_inhibit_manager_v1_requests: [wl_message; 1] = [wl_message {
        name: b"get_inhibitor\0" as *const u8 as *const c_char,
        signature: b"n\0" as *const u8 as *const c_char,
        types: unsafe { &zwlr_input_inhibit_manager_v1_requests_get_inhibitor_types as *const _ },
    }];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwlr_input_inhibit_manager_v1_interface: wl_interface = wl_interface {
        name: b"zwlr_input_inhibit_manager_v1\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 1,
        requests: unsafe { &zwlr_input_inhibit_manager_v1_requests as *const _ },
        event_count: 0,
        events: NULLPTR as *const wl_message,
    };
}
#[doc = "inhibits input to other clients\n\nWhile this resource exists, input to clients other than the owner of the\ninhibitor resource will not receive input events. Any client which\npreviously had focus will receive a leave event and will not be given\nfocus again. The client that owns this resource will receive all input\nevents normally. The compositor will also disable all of its own input\nprocessing (such as keyboard shortcuts) while the inhibitor is active.\n\nThe compositor may continue to send input events to selected clients,\nsuch as an on-screen keyboard (via the input-method protocol)."]
pub mod zwlr_input_inhibitor_v1 {
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
        #[doc = "destroy the input inhibitor object\n\nDestroy the inhibitor and allow other clients to receive input.\n\nThis is a destructor, once received this object cannot be used any longer."]
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
    pub struct ZwlrInputInhibitorV1(Resource<ZwlrInputInhibitorV1>);
    impl AsRef<Resource<ZwlrInputInhibitorV1>> for ZwlrInputInhibitorV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwlrInputInhibitorV1>> for ZwlrInputInhibitorV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwlrInputInhibitorV1(value)
        }
    }
    impl From<ZwlrInputInhibitorV1> for Resource<ZwlrInputInhibitorV1> {
        #[inline]
        fn from(value: ZwlrInputInhibitorV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwlrInputInhibitorV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwlrInputInhibitorV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwlr_input_inhibitor_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwlr_input_inhibitor_v1_interface }
        }
    }
    impl ZwlrInputInhibitorV1 {}
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwlr_input_inhibitor_v1_requests: [wl_message; 1] = [wl_message {
        name: b"destroy\0" as *const u8 as *const c_char,
        signature: b"\0" as *const u8 as *const c_char,
        types: unsafe { &types_null as *const _ },
    }];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwlr_input_inhibitor_v1_interface: wl_interface = wl_interface {
        name: b"zwlr_input_inhibitor_v1\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 1,
        requests: unsafe { &zwlr_input_inhibitor_v1_requests as *const _ },
        event_count: 0,
        events: NULLPTR as *const wl_message,
    };
}
