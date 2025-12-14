use std::os::raw::{c_char, c_void};
const NULLPTR: *const c_void = 0 as *const c_void;
static mut types_null: [*const sys::common::wl_interface; 0] = [];
#[doc = "context object for keyboard grab_manager\n\nA global interface used for inhibiting the compositor keyboard shortcuts."]
pub mod zwp_keyboard_shortcuts_inhibit_manager_v1 {
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
        #[doc = "the shortcuts are already inhibited for this surface"]
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
        #[doc = "destroy the keyboard shortcuts inhibitor object\n\nDestroy the keyboard shortcuts inhibitor manager.\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
        #[doc = "create a new keyboard shortcuts inhibitor object\n\nCreate a new keyboard shortcuts inhibitor object associated with\nthe given surface for the given seat.\n\nIf shortcuts are already inhibited for the specified seat and surface,\na protocol error \"already_inhibited\" is raised by the compositor."]
        InhibitShortcuts {
            id: Main<super::zwp_keyboard_shortcuts_inhibitor_v1::ZwpKeyboardShortcutsInhibitorV1>,
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
                name: "inhibit_shortcuts",
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
                Request::InhibitShortcuts { .. } => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::Destroy => 1,
                Request::InhibitShortcuts { .. } => 1,
            }
        }
        fn child<Meta: ObjectMetadata>(
            opcode: u16,
            version: u32,
            meta: &Meta,
        ) -> Option<Object<Meta>> {
            match opcode {
                1 => Some(Object::from_interface::<
                    super::zwp_keyboard_shortcuts_inhibitor_v1::ZwpKeyboardShortcutsInhibitorV1,
                >(version, meta.child())),
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            match msg.opcode {
                0 => Ok(Request::Destroy),
                1 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::InhibitShortcuts {
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
                    Ok(Request::InhibitShortcuts {
                        id: {
                            let me = Resource::<ZwpKeyboardShortcutsInhibitManagerV1>::from_c_ptr(
                                obj as *mut _,
                            );
                            me . make_child_for :: < super :: zwp_keyboard_shortcuts_inhibitor_v1 :: ZwpKeyboardShortcutsInhibitorV1 > (_args [0] . n) . unwrap ()
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
    pub struct ZwpKeyboardShortcutsInhibitManagerV1(Resource<ZwpKeyboardShortcutsInhibitManagerV1>);
    impl AsRef<Resource<ZwpKeyboardShortcutsInhibitManagerV1>>
        for ZwpKeyboardShortcutsInhibitManagerV1
    {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwpKeyboardShortcutsInhibitManagerV1>> for ZwpKeyboardShortcutsInhibitManagerV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwpKeyboardShortcutsInhibitManagerV1(value)
        }
    }
    impl From<ZwpKeyboardShortcutsInhibitManagerV1> for Resource<ZwpKeyboardShortcutsInhibitManagerV1> {
        #[inline]
        fn from(value: ZwpKeyboardShortcutsInhibitManagerV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwpKeyboardShortcutsInhibitManagerV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwpKeyboardShortcutsInhibitManagerV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwp_keyboard_shortcuts_inhibit_manager_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwp_keyboard_shortcuts_inhibit_manager_v1_interface }
        }
    }
    impl ZwpKeyboardShortcutsInhibitManagerV1 {}
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_INHIBIT_SHORTCUTS_SINCE: u32 = 1u32;
    static mut zwp_keyboard_shortcuts_inhibit_manager_v1_requests_inhibit_shortcuts_types:
        [*const wl_interface; 3] = [
        unsafe {
            & super :: zwp_keyboard_shortcuts_inhibitor_v1 :: zwp_keyboard_shortcuts_inhibitor_v1_interface as * const wl_interface
        },
        unsafe { &super::wl_surface::wl_surface_interface as *const wl_interface },
        unsafe { &super::wl_seat::wl_seat_interface as *const wl_interface },
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_keyboard_shortcuts_inhibit_manager_v1_requests: [wl_message; 2] = [
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"inhibit_shortcuts\0" as *const u8 as *const c_char,
            signature: b"noo\0" as *const u8 as *const c_char,
            types: unsafe {
                &zwp_keyboard_shortcuts_inhibit_manager_v1_requests_inhibit_shortcuts_types
                    as *const _
            },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwp_keyboard_shortcuts_inhibit_manager_v1_interface: wl_interface =
        wl_interface {
            name: b"zwp_keyboard_shortcuts_inhibit_manager_v1\0" as *const u8 as *const c_char,
            version: 1,
            request_count: 2,
            requests: unsafe { &zwp_keyboard_shortcuts_inhibit_manager_v1_requests as *const _ },
            event_count: 0,
            events: NULLPTR as *const wl_message,
        };
}
#[doc = "context object for keyboard shortcuts inhibitor\n\nA keyboard shortcuts inhibitor instructs the compositor to ignore\nits own keyboard shortcuts when the associated surface has keyboard\nfocus. As a result, when the surface has keyboard focus on the given\nseat, it will receive all key events originating from the specified\nseat, even those which would normally be caught by the compositor for\nits own shortcuts.\n\nThe Wayland compositor is however under no obligation to disable\nall of its shortcuts, and may keep some special key combo for its own\nuse, including but not limited to one allowing the user to forcibly\nrestore normal keyboard events routing in the case of an unwilling\nclient. The compositor may also use the same key combo to reactivate\nan existing shortcut inhibitor that was previously deactivated on\nuser request.\n\nWhen the compositor restores its own keyboard shortcuts, an\n\"inactive\" event is emitted to notify the client that the keyboard\nshortcuts inhibitor is not effectively active for the surface and\nseat any more, and the client should not expect to receive all\nkeyboard events.\n\nWhen the keyboard shortcuts inhibitor is inactive, the client has\nno way to forcibly reactivate the keyboard shortcuts inhibitor.\n\nThe user can chose to re-enable a previously deactivated keyboard\nshortcuts inhibitor using any mechanism the compositor may offer,\nin which case the compositor will send an \"active\" event to notify\nthe client.\n\nIf the surface is destroyed, unmapped, or loses the seat's keyboard\nfocus, the keyboard shortcuts inhibitor becomes irrelevant and the\ncompositor will restore its own keyboard shortcuts but no \"inactive\"\nevent is emitted in this case."]
pub mod zwp_keyboard_shortcuts_inhibitor_v1 {
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
        #[doc = "destroy the keyboard shortcuts inhibitor object\n\nRemove the keyboard shortcuts inhibitor from the associated wl_surface.\n\nThis is a destructor, once received this object cannot be used any longer."]
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
        #[doc = "shortcuts are inhibited\n\nThis event indicates that the shortcut inhibitor is active.\n\nThe compositor sends this event every time compositor shortcuts\nare inhibited on behalf of the surface. When active, the client\nmay receive input events normally reserved by the compositor\n(see zwp_keyboard_shortcuts_inhibitor_v1).\n\nThis occurs typically when the initial request \"inhibit_shortcuts\"\nfirst becomes active or when the user instructs the compositor to\nre-enable and existing shortcuts inhibitor using any mechanism\noffered by the compositor."]
        Active,
        #[doc = "shortcuts are restored\n\nThis event indicates that the shortcuts inhibitor is inactive,\nnormal shortcuts processing is restored by the compositor."]
        Inactive,
    }
    impl super::MessageGroup for Event {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "active",
                since: 1,
                signature: &[],
                destructor: false,
            },
            super::MessageDesc {
                name: "inactive",
                since: 1,
                signature: &[],
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
                Event::Active => 0,
                Event::Inactive => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Event::Active => 1,
                Event::Inactive => 1,
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
                Event::Active => Message {
                    sender_id: sender_id,
                    opcode: 0,
                    args: smallvec![],
                },
                Event::Inactive => Message {
                    sender_id: sender_id,
                    opcode: 1,
                    args: smallvec![],
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
                Event::Active => {
                    let mut _args_array: [wl_argument; 0] = unsafe { ::std::mem::zeroed() };
                    f(0, &mut _args_array)
                }
                Event::Inactive => {
                    let mut _args_array: [wl_argument; 0] = unsafe { ::std::mem::zeroed() };
                    f(1, &mut _args_array)
                }
            }
        }
    }
    #[derive(Clone, Eq, PartialEq)]
    pub struct ZwpKeyboardShortcutsInhibitorV1(Resource<ZwpKeyboardShortcutsInhibitorV1>);
    impl AsRef<Resource<ZwpKeyboardShortcutsInhibitorV1>> for ZwpKeyboardShortcutsInhibitorV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwpKeyboardShortcutsInhibitorV1>> for ZwpKeyboardShortcutsInhibitorV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwpKeyboardShortcutsInhibitorV1(value)
        }
    }
    impl From<ZwpKeyboardShortcutsInhibitorV1> for Resource<ZwpKeyboardShortcutsInhibitorV1> {
        #[inline]
        fn from(value: ZwpKeyboardShortcutsInhibitorV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwpKeyboardShortcutsInhibitorV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwpKeyboardShortcutsInhibitorV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwp_keyboard_shortcuts_inhibitor_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwp_keyboard_shortcuts_inhibitor_v1_interface }
        }
    }
    impl ZwpKeyboardShortcutsInhibitorV1 {
        #[doc = "shortcuts are inhibited\n\nThis event indicates that the shortcut inhibitor is active.\n\nThe compositor sends this event every time compositor shortcuts\nare inhibited on behalf of the surface. When active, the client\nmay receive input events normally reserved by the compositor\n(see zwp_keyboard_shortcuts_inhibitor_v1).\n\nThis occurs typically when the initial request \"inhibit_shortcuts\"\nfirst becomes active or when the user instructs the compositor to\nre-enable and existing shortcuts inhibitor using any mechanism\noffered by the compositor."]
        pub fn active(&self) -> () {
            let msg = Event::Active;
            self.0.send(msg);
        }
        #[doc = "shortcuts are restored\n\nThis event indicates that the shortcuts inhibitor is inactive,\nnormal shortcuts processing is restored by the compositor."]
        pub fn inactive(&self) -> () {
            let msg = Event::Inactive;
            self.0.send(msg);
        }
    }
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_ACTIVE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_INACTIVE_SINCE: u32 = 1u32;
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_keyboard_shortcuts_inhibitor_v1_requests: [wl_message; 1] = [wl_message {
        name: b"destroy\0" as *const u8 as *const c_char,
        signature: b"\0" as *const u8 as *const c_char,
        types: unsafe { &types_null as *const _ },
    }];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_keyboard_shortcuts_inhibitor_v1_events: [wl_message; 2] = [
        wl_message {
            name: b"active\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"inactive\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwp_keyboard_shortcuts_inhibitor_v1_interface: wl_interface = wl_interface {
        name: b"zwp_keyboard_shortcuts_inhibitor_v1\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 1,
        requests: unsafe { &zwp_keyboard_shortcuts_inhibitor_v1_requests as *const _ },
        event_count: 2,
        events: unsafe { &zwp_keyboard_shortcuts_inhibitor_v1_events as *const _ },
    };
}
