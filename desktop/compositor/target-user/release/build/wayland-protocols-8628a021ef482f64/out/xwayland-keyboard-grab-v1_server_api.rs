use std::os::raw::{c_char, c_void};
const NULLPTR: *const c_void = 0 as *const c_void;
static mut types_null: [*const sys::common::wl_interface; 0] = [];
#[doc = "context object for keyboard grab manager\n\nA global interface used for grabbing the keyboard."]
pub mod zwp_xwayland_keyboard_grab_manager_v1 {
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
        #[doc = "destroy the keyboard grab manager\n\nDestroy the keyboard grab manager.\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
        #[doc = "grab the keyboard to a surface\n\nThe grab_keyboard request asks for a grab of the keyboard, forcing\nthe keyboard focus for the given seat upon the given surface.\n\nThe protocol provides no guarantee that the grab is ever satisfied,\nand does not require the compositor to send an error if the grab\ncannot ever be satisfied. It is thus possible to request a keyboard\ngrab that will never be effective.\n\nThe protocol:\n\n* does not guarantee that the grab itself is applied for a surface,\nthe grab request may be silently ignored by the compositor,\n* does not guarantee that any events are sent to this client even\nif the grab is applied to a surface,\n* does not guarantee that events sent to this client are exhaustive,\na compositor may filter some events for its own consumption,\n* does not guarantee that events sent to this client are continuous,\na compositor may change and reroute keyboard events while the grab\nis nominally active."]
        GrabKeyboard {
            id: Main<super::zwp_xwayland_keyboard_grab_v1::ZwpXwaylandKeyboardGrabV1>,
            surface: super::wl_surface::WlSurface,
            seat: super::wl_seat::WlSeat,
        },
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "destroy",
                since: 1,
                signature: &[],
                destructor: true,
            },
            super::MessageDesc {
                name: "grab_keyboard",
                since: 1,
                signature: &[
                    super::ArgumentType::NewId,
                    super::ArgumentType::Object,
                    super::ArgumentType::Object,
                ],
                destructor: false,
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
                Request::Destroy => 0,
                Request::GrabKeyboard { .. } => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::Destroy => 1,
                Request::GrabKeyboard { .. } => 1,
            }
        }
        fn child<Meta: ObjectMetadata>(
            opcode: u16,
            version: u32,
            meta: &Meta,
        ) -> Option<Object<Meta>> {
            match opcode {
                1 => Some(Object::from_interface::<
                    super::zwp_xwayland_keyboard_grab_v1::ZwpXwaylandKeyboardGrabV1,
                >(version, meta.child())),
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            match msg.opcode {
                0 => Ok(Request::Destroy),
                1 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::GrabKeyboard {
                        id: {
                            if let Some(Argument::NewId(val)) = args.next() {
                                map.get_new(val).ok_or(())?
                            } else {
                                return Err(());
                            }
                        },
                        surface: {
                            if let Some(Argument::Object(val)) = args.next() {
                                map.get(val).ok_or(())?.into()
                            } else {
                                return Err(());
                            }
                        },
                        seat: {
                            if let Some(Argument::Object(val)) = args.next() {
                                map.get(val).ok_or(())?.into()
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
                0 => Ok(Request::Destroy),
                1 => {
                    let _args = ::std::slice::from_raw_parts(args, 3);
                    Ok(Request::GrabKeyboard {
                        id: {
                            let me = Resource::<ZwpXwaylandKeyboardGrabManagerV1>::from_c_ptr(
                                obj as *mut _,
                            );
                            me . make_child_for :: < super :: zwp_xwayland_keyboard_grab_v1 :: ZwpXwaylandKeyboardGrabV1 > (_args [0] . n) . unwrap ()
                        },
                        surface: Resource::<super::wl_surface::WlSurface>::from_c_ptr(
                            _args[1].o as *mut _,
                        )
                        .into(),
                        seat: Resource::<super::wl_seat::WlSeat>::from_c_ptr(_args[2].o as *mut _)
                            .into(),
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
    pub struct ZwpXwaylandKeyboardGrabManagerV1(Resource<ZwpXwaylandKeyboardGrabManagerV1>);
    impl AsRef<Resource<ZwpXwaylandKeyboardGrabManagerV1>> for ZwpXwaylandKeyboardGrabManagerV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwpXwaylandKeyboardGrabManagerV1>> for ZwpXwaylandKeyboardGrabManagerV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwpXwaylandKeyboardGrabManagerV1(value)
        }
    }
    impl From<ZwpXwaylandKeyboardGrabManagerV1> for Resource<ZwpXwaylandKeyboardGrabManagerV1> {
        #[inline]
        fn from(value: ZwpXwaylandKeyboardGrabManagerV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwpXwaylandKeyboardGrabManagerV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwpXwaylandKeyboardGrabManagerV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwp_xwayland_keyboard_grab_manager_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwp_xwayland_keyboard_grab_manager_v1_interface }
        }
    }
    impl ZwpXwaylandKeyboardGrabManagerV1 {}
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_GRAB_KEYBOARD_SINCE: u32 = 1u32;
    static mut zwp_xwayland_keyboard_grab_manager_v1_requests_grab_keyboard_types:
        [*const wl_interface; 3] = [
        unsafe {
            &super::zwp_xwayland_keyboard_grab_v1::zwp_xwayland_keyboard_grab_v1_interface
                as *const wl_interface
        },
        unsafe { &super::wl_surface::wl_surface_interface as *const wl_interface },
        unsafe { &super::wl_seat::wl_seat_interface as *const wl_interface },
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_xwayland_keyboard_grab_manager_v1_requests: [wl_message; 2] = [
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"grab_keyboard\0" as *const u8 as *const c_char,
            signature: b"noo\0" as *const u8 as *const c_char,
            types: unsafe {
                &zwp_xwayland_keyboard_grab_manager_v1_requests_grab_keyboard_types as *const _
            },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwp_xwayland_keyboard_grab_manager_v1_interface: wl_interface = wl_interface {
        name: b"zwp_xwayland_keyboard_grab_manager_v1\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 2,
        requests: unsafe { &zwp_xwayland_keyboard_grab_manager_v1_requests as *const _ },
        event_count: 0,
        events: NULLPTR as *const wl_message,
    };
}
#[doc = "interface for grabbing the keyboard\n\nA global interface used for grabbing the keyboard."]
pub mod zwp_xwayland_keyboard_grab_v1 {
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
        #[doc = "destroy the grabbed keyboard object\n\nDestroy the grabbed keyboard object. If applicable, the compositor\nwill ungrab the keyboard.\n\nThis is a destructor, once received this object cannot be used any longer."]
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
    pub struct ZwpXwaylandKeyboardGrabV1(Resource<ZwpXwaylandKeyboardGrabV1>);
    impl AsRef<Resource<ZwpXwaylandKeyboardGrabV1>> for ZwpXwaylandKeyboardGrabV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwpXwaylandKeyboardGrabV1>> for ZwpXwaylandKeyboardGrabV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwpXwaylandKeyboardGrabV1(value)
        }
    }
    impl From<ZwpXwaylandKeyboardGrabV1> for Resource<ZwpXwaylandKeyboardGrabV1> {
        #[inline]
        fn from(value: ZwpXwaylandKeyboardGrabV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwpXwaylandKeyboardGrabV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwpXwaylandKeyboardGrabV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwp_xwayland_keyboard_grab_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwp_xwayland_keyboard_grab_v1_interface }
        }
    }
    impl ZwpXwaylandKeyboardGrabV1 {}
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_xwayland_keyboard_grab_v1_requests: [wl_message; 1] = [wl_message {
        name: b"destroy\0" as *const u8 as *const c_char,
        signature: b"\0" as *const u8 as *const c_char,
        types: unsafe { &types_null as *const _ },
    }];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwp_xwayland_keyboard_grab_v1_interface: wl_interface = wl_interface {
        name: b"zwp_xwayland_keyboard_grab_v1\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 1,
        requests: unsafe { &zwp_xwayland_keyboard_grab_v1_requests as *const _ },
        event_count: 0,
        events: NULLPTR as *const wl_message,
    };
}
