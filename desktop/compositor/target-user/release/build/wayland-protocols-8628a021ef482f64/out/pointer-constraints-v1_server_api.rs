use std::os::raw::{c_char, c_void};
const NULLPTR: *const c_void = 0 as *const c_void;
static mut types_null: [*const sys::common::wl_interface; 2] = [
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
];
#[doc = "constrain the movement of a pointer\n\nThe global interface exposing pointer constraining functionality. It\nexposes two requests: lock_pointer for locking the pointer to its\nposition, and confine_pointer for locking the pointer to a region.\n\nThe lock_pointer and confine_pointer requests create the objects\nwp_locked_pointer and wp_confined_pointer respectively, and the client can\nuse these objects to interact with the lock.\n\nFor any surface, only one lock or confinement may be active across all\nwl_pointer objects of the same seat. If a lock or confinement is requested\nwhen another lock or confinement is active or requested on the same surface\nand with any of the wl_pointer objects of the same seat, an\n'already_constrained' error will be raised."]
pub mod zwp_pointer_constraints_v1 {
    use super::sys::common::{wl_argument, wl_array, wl_interface, wl_message};
    use super::sys::server::*;
    use super::{
        smallvec, types_null, AnonymousObject, Argument, ArgumentType, Interface, Main, Message,
        MessageDesc, MessageGroup, Object, ObjectMetadata, Resource, NULLPTR,
    };
    use std::os::raw::c_char;
    #[doc = "wp_pointer_constraints error values\n\nThese errors can be emitted in response to wp_pointer_constraints\nrequests."]
    #[repr(u32)]
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[non_exhaustive]
    pub enum Error {
        #[doc = "pointer constraint already requested on that surface"]
        AlreadyConstrained = 1,
    }
    impl Error {
        pub fn from_raw(n: u32) -> Option<Error> {
            match n {
                1 => Some(Error::AlreadyConstrained),
                _ => Option::None,
            }
        }
        pub fn to_raw(&self) -> u32 {
            *self as u32
        }
    }
    #[doc = "constraint lifetime\n\nThese values represent different lifetime semantics. They are passed\nas arguments to the factory requests to specify how the constraint\nlifetimes should be managed."]
    #[repr(u32)]
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[non_exhaustive]
    pub enum Lifetime {
        #[doc = "the pointer constraint is defunct once deactivated\n\nA oneshot pointer constraint will never reactivate once it has been\ndeactivated. See the corresponding deactivation event\n(wp_locked_pointer.unlocked and wp_confined_pointer.unconfined) for\ndetails."]
        Oneshot = 1,
        #[doc = "the pointer constraint may reactivate\n\nA persistent pointer constraint may again reactivate once it has\nbeen deactivated. See the corresponding deactivation event\n(wp_locked_pointer.unlocked and wp_confined_pointer.unconfined) for\ndetails."]
        Persistent = 2,
    }
    impl Lifetime {
        pub fn from_raw(n: u32) -> Option<Lifetime> {
            match n {
                1 => Some(Lifetime::Oneshot),
                2 => Some(Lifetime::Persistent),
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
        #[doc = "destroy the pointer constraints manager object\n\nUsed by the client to notify the server that it will no longer use this\npointer constraints object.\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
        #[doc = "lock pointer to a position\n\nThe lock_pointer request lets the client request to disable movements of\nthe virtual pointer (i.e. the cursor), effectively locking the pointer\nto a position. This request may not take effect immediately; in the\nfuture, when the compositor deems implementation-specific constraints\nare satisfied, the pointer lock will be activated and the compositor\nsends a locked event.\n\nThe protocol provides no guarantee that the constraints are ever\nsatisfied, and does not require the compositor to send an error if the\nconstraints cannot ever be satisfied. It is thus possible to request a\nlock that will never activate.\n\nThere may not be another pointer constraint of any kind requested or\nactive on the surface for any of the wl_pointer objects of the seat of\nthe passed pointer when requesting a lock. If there is, an error will be\nraised. See general pointer lock documentation for more details.\n\nThe intersection of the region passed with this request and the input\nregion of the surface is used to determine where the pointer must be\nin order for the lock to activate. It is up to the compositor whether to\nwarp the pointer or require some kind of user interaction for the lock\nto activate. If the region is null the surface input region is used.\n\nA surface may receive pointer focus without the lock being activated.\n\nThe request creates a new object wp_locked_pointer which is used to\ninteract with the lock as well as receive updates about its state. See\nthe the description of wp_locked_pointer for further information.\n\nNote that while a pointer is locked, the wl_pointer objects of the\ncorresponding seat will not emit any wl_pointer.motion events, but\nrelative motion events will still be emitted via wp_relative_pointer\nobjects of the same seat. wl_pointer.axis and wl_pointer.button events\nare unaffected."]
        LockPointer {
            id: Main<super::zwp_locked_pointer_v1::ZwpLockedPointerV1>,
            surface: super::wl_surface::WlSurface,
            pointer: super::wl_pointer::WlPointer,
            region: Option<super::wl_region::WlRegion>,
            lifetime: Lifetime,
        },
        #[doc = "confine pointer to a region\n\nThe confine_pointer request lets the client request to confine the\npointer cursor to a given region. This request may not take effect\nimmediately; in the future, when the compositor deems implementation-\nspecific constraints are satisfied, the pointer confinement will be\nactivated and the compositor sends a confined event.\n\nThe intersection of the region passed with this request and the input\nregion of the surface is used to determine where the pointer must be\nin order for the confinement to activate. It is up to the compositor\nwhether to warp the pointer or require some kind of user interaction for\nthe confinement to activate. If the region is null the surface input\nregion is used.\n\nThe request will create a new object wp_confined_pointer which is used\nto interact with the confinement as well as receive updates about its\nstate. See the the description of wp_confined_pointer for further\ninformation."]
        ConfinePointer {
            id: Main<super::zwp_confined_pointer_v1::ZwpConfinedPointerV1>,
            surface: super::wl_surface::WlSurface,
            pointer: super::wl_pointer::WlPointer,
            region: Option<super::wl_region::WlRegion>,
            lifetime: Lifetime,
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
                name: "lock_pointer",
                since: 1,
                signature: &[
                    super::ArgumentType::NewId,
                    super::ArgumentType::Object,
                    super::ArgumentType::Object,
                    super::ArgumentType::Object,
                    super::ArgumentType::Uint,
                ],
                destructor: false,
            },
            super::MessageDesc {
                name: "confine_pointer",
                since: 1,
                signature: &[
                    super::ArgumentType::NewId,
                    super::ArgumentType::Object,
                    super::ArgumentType::Object,
                    super::ArgumentType::Object,
                    super::ArgumentType::Uint,
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
                Request::LockPointer { .. } => 1,
                Request::ConfinePointer { .. } => 2,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::Destroy => 1,
                Request::LockPointer { .. } => 1,
                Request::ConfinePointer { .. } => 1,
            }
        }
        fn child<Meta: ObjectMetadata>(
            opcode: u16,
            version: u32,
            meta: &Meta,
        ) -> Option<Object<Meta>> {
            match opcode {
                1 => Some(Object::from_interface::<
                    super::zwp_locked_pointer_v1::ZwpLockedPointerV1,
                >(version, meta.child())),
                2 => Some(Object::from_interface::<
                    super::zwp_confined_pointer_v1::ZwpConfinedPointerV1,
                >(version, meta.child())),
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            match msg.opcode {
                0 => Ok(Request::Destroy),
                1 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::LockPointer {
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
                        pointer: {
                            if let Some(Argument::Object(val)) = args.next() {
                                map.get(val).ok_or(())?.into()
                            } else {
                                return Err(());
                            }
                        },
                        region: {
                            if let Some(Argument::Object(val)) = args.next() {
                                if val == 0 {
                                    None
                                } else {
                                    Some(map.get(val).ok_or(())?.into())
                                }
                            } else {
                                return Err(());
                            }
                        },
                        lifetime: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                Lifetime::from_raw(val).ok_or(())?
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                2 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::ConfinePointer {
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
                        pointer: {
                            if let Some(Argument::Object(val)) = args.next() {
                                map.get(val).ok_or(())?.into()
                            } else {
                                return Err(());
                            }
                        },
                        region: {
                            if let Some(Argument::Object(val)) = args.next() {
                                if val == 0 {
                                    None
                                } else {
                                    Some(map.get(val).ok_or(())?.into())
                                }
                            } else {
                                return Err(());
                            }
                        },
                        lifetime: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                Lifetime::from_raw(val).ok_or(())?
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
                    let _args = ::std::slice::from_raw_parts(args, 5);
                    Ok(Request::LockPointer {
                        id: {
                            let me = Resource::<ZwpPointerConstraintsV1>::from_c_ptr(obj as *mut _);
                            me.make_child_for::<super::zwp_locked_pointer_v1::ZwpLockedPointerV1>(
                                _args[0].n,
                            )
                            .unwrap()
                        },
                        surface: Resource::<super::wl_surface::WlSurface>::from_c_ptr(
                            _args[1].o as *mut _,
                        )
                        .into(),
                        pointer: Resource::<super::wl_pointer::WlPointer>::from_c_ptr(
                            _args[2].o as *mut _,
                        )
                        .into(),
                        region: if _args[3].o.is_null() {
                            None
                        } else {
                            Some(
                                Resource::<super::wl_region::WlRegion>::from_c_ptr(
                                    _args[3].o as *mut _,
                                )
                                .into(),
                            )
                        },
                        lifetime: Lifetime::from_raw(_args[4].u).ok_or(())?,
                    })
                }
                2 => {
                    let _args = ::std::slice::from_raw_parts(args, 5);
                    Ok(Request::ConfinePointer {
                        id: {
                            let me = Resource::<ZwpPointerConstraintsV1>::from_c_ptr(obj as *mut _);
                            me . make_child_for :: < super :: zwp_confined_pointer_v1 :: ZwpConfinedPointerV1 > (_args [0] . n) . unwrap ()
                        },
                        surface: Resource::<super::wl_surface::WlSurface>::from_c_ptr(
                            _args[1].o as *mut _,
                        )
                        .into(),
                        pointer: Resource::<super::wl_pointer::WlPointer>::from_c_ptr(
                            _args[2].o as *mut _,
                        )
                        .into(),
                        region: if _args[3].o.is_null() {
                            None
                        } else {
                            Some(
                                Resource::<super::wl_region::WlRegion>::from_c_ptr(
                                    _args[3].o as *mut _,
                                )
                                .into(),
                            )
                        },
                        lifetime: Lifetime::from_raw(_args[4].u).ok_or(())?,
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
    pub struct ZwpPointerConstraintsV1(Resource<ZwpPointerConstraintsV1>);
    impl AsRef<Resource<ZwpPointerConstraintsV1>> for ZwpPointerConstraintsV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwpPointerConstraintsV1>> for ZwpPointerConstraintsV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwpPointerConstraintsV1(value)
        }
    }
    impl From<ZwpPointerConstraintsV1> for Resource<ZwpPointerConstraintsV1> {
        #[inline]
        fn from(value: ZwpPointerConstraintsV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwpPointerConstraintsV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwpPointerConstraintsV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwp_pointer_constraints_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwp_pointer_constraints_v1_interface }
        }
    }
    impl ZwpPointerConstraintsV1 {}
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_LOCK_POINTER_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_CONFINE_POINTER_SINCE: u32 = 1u32;
    static mut zwp_pointer_constraints_v1_requests_lock_pointer_types: [*const wl_interface; 5] = [
        unsafe {
            &super::zwp_locked_pointer_v1::zwp_locked_pointer_v1_interface as *const wl_interface
        },
        unsafe { &super::wl_surface::wl_surface_interface as *const wl_interface },
        unsafe { &super::wl_pointer::wl_pointer_interface as *const wl_interface },
        unsafe { &super::wl_region::wl_region_interface as *const wl_interface },
        NULLPTR as *const wl_interface,
    ];
    static mut zwp_pointer_constraints_v1_requests_confine_pointer_types: [*const wl_interface; 5] = [
        unsafe {
            &super::zwp_confined_pointer_v1::zwp_confined_pointer_v1_interface
                as *const wl_interface
        },
        unsafe { &super::wl_surface::wl_surface_interface as *const wl_interface },
        unsafe { &super::wl_pointer::wl_pointer_interface as *const wl_interface },
        unsafe { &super::wl_region::wl_region_interface as *const wl_interface },
        NULLPTR as *const wl_interface,
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_pointer_constraints_v1_requests: [wl_message; 3] = [
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"lock_pointer\0" as *const u8 as *const c_char,
            signature: b"noo?ou\0" as *const u8 as *const c_char,
            types: unsafe { &zwp_pointer_constraints_v1_requests_lock_pointer_types as *const _ },
        },
        wl_message {
            name: b"confine_pointer\0" as *const u8 as *const c_char,
            signature: b"noo?ou\0" as *const u8 as *const c_char,
            types: unsafe {
                &zwp_pointer_constraints_v1_requests_confine_pointer_types as *const _
            },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwp_pointer_constraints_v1_interface: wl_interface = wl_interface {
        name: b"zwp_pointer_constraints_v1\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 3,
        requests: unsafe { &zwp_pointer_constraints_v1_requests as *const _ },
        event_count: 0,
        events: NULLPTR as *const wl_message,
    };
}
#[doc = "receive relative pointer motion events\n\nThe wp_locked_pointer interface represents a locked pointer state.\n\nWhile the lock of this object is active, the wl_pointer objects of the\nassociated seat will not emit any wl_pointer.motion events.\n\nThis object will send the event 'locked' when the lock is activated.\nWhenever the lock is activated, it is guaranteed that the locked surface\nwill already have received pointer focus and that the pointer will be\nwithin the region passed to the request creating this object.\n\nTo unlock the pointer, send the destroy request. This will also destroy\nthe wp_locked_pointer object.\n\nIf the compositor decides to unlock the pointer the unlocked event is\nsent. See wp_locked_pointer.unlock for details.\n\nWhen unlocking, the compositor may warp the cursor position to the set\ncursor position hint. If it does, it will not result in any relative\nmotion events emitted via wp_relative_pointer.\n\nIf the surface the lock was requested on is destroyed and the lock is not\nyet activated, the wp_locked_pointer object is now defunct and must be\ndestroyed."]
pub mod zwp_locked_pointer_v1 {
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
        #[doc = "destroy the locked pointer object\n\nDestroy the locked pointer object. If applicable, the compositor will\nunlock the pointer.\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
        #[doc = "set the pointer cursor position hint\n\nSet the cursor position hint relative to the top left corner of the\nsurface.\n\nIf the client is drawing its own cursor, it should update the position\nhint to the position of its own cursor. A compositor may use this\ninformation to warp the pointer upon unlock in order to avoid pointer\njumps.\n\nThe cursor position hint is double buffered. The new hint will only take\neffect when the associated surface gets it pending state applied. See\nwl_surface.commit for details."]
        SetCursorPositionHint { surface_x: f64, surface_y: f64 },
        #[doc = "set a new lock region\n\nSet a new region used to lock the pointer.\n\nThe new lock region is double-buffered. The new lock region will\nonly take effect when the associated surface gets its pending state\napplied. See wl_surface.commit for details.\n\nFor details about the lock region, see wp_locked_pointer."]
        SetRegion {
            region: Option<super::wl_region::WlRegion>,
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
                name: "set_cursor_position_hint",
                since: 1,
                signature: &[super::ArgumentType::Fixed, super::ArgumentType::Fixed],
                destructor: false,
            },
            super::MessageDesc {
                name: "set_region",
                since: 1,
                signature: &[super::ArgumentType::Object],
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
                Request::SetCursorPositionHint { .. } => 1,
                Request::SetRegion { .. } => 2,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::Destroy => 1,
                Request::SetCursorPositionHint { .. } => 1,
                Request::SetRegion { .. } => 1,
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
                1 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::SetCursorPositionHint {
                        surface_x: {
                            if let Some(Argument::Fixed(val)) = args.next() {
                                (val as f64) / 256.
                            } else {
                                return Err(());
                            }
                        },
                        surface_y: {
                            if let Some(Argument::Fixed(val)) = args.next() {
                                (val as f64) / 256.
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                2 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::SetRegion {
                        region: {
                            if let Some(Argument::Object(val)) = args.next() {
                                if val == 0 {
                                    None
                                } else {
                                    Some(map.get(val).ok_or(())?.into())
                                }
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
                    let _args = ::std::slice::from_raw_parts(args, 2);
                    Ok(Request::SetCursorPositionHint {
                        surface_x: (_args[0].f as f64) / 256.,
                        surface_y: (_args[1].f as f64) / 256.,
                    })
                }
                2 => {
                    let _args = ::std::slice::from_raw_parts(args, 1);
                    Ok(Request::SetRegion {
                        region: if _args[0].o.is_null() {
                            None
                        } else {
                            Some(
                                Resource::<super::wl_region::WlRegion>::from_c_ptr(
                                    _args[0].o as *mut _,
                                )
                                .into(),
                            )
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
    pub enum Event {
        #[doc = "lock activation event\n\nNotification that the pointer lock of the seat's pointer is activated."]
        Locked,
        #[doc = "lock deactivation event\n\nNotification that the pointer lock of the seat's pointer is no longer\nactive. If this is a oneshot pointer lock (see\nwp_pointer_constraints.lifetime) this object is now defunct and should\nbe destroyed. If this is a persistent pointer lock (see\nwp_pointer_constraints.lifetime) this pointer lock may again\nreactivate in the future."]
        Unlocked,
    }
    impl super::MessageGroup for Event {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "locked",
                since: 1,
                signature: &[],
                destructor: false,
            },
            super::MessageDesc {
                name: "unlocked",
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
                Event::Locked => 0,
                Event::Unlocked => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Event::Locked => 1,
                Event::Unlocked => 1,
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
                Event::Locked => Message {
                    sender_id: sender_id,
                    opcode: 0,
                    args: smallvec![],
                },
                Event::Unlocked => Message {
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
                Event::Locked => {
                    let mut _args_array: [wl_argument; 0] = unsafe { ::std::mem::zeroed() };
                    f(0, &mut _args_array)
                }
                Event::Unlocked => {
                    let mut _args_array: [wl_argument; 0] = unsafe { ::std::mem::zeroed() };
                    f(1, &mut _args_array)
                }
            }
        }
    }
    #[derive(Clone, Eq, PartialEq)]
    pub struct ZwpLockedPointerV1(Resource<ZwpLockedPointerV1>);
    impl AsRef<Resource<ZwpLockedPointerV1>> for ZwpLockedPointerV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwpLockedPointerV1>> for ZwpLockedPointerV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwpLockedPointerV1(value)
        }
    }
    impl From<ZwpLockedPointerV1> for Resource<ZwpLockedPointerV1> {
        #[inline]
        fn from(value: ZwpLockedPointerV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwpLockedPointerV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwpLockedPointerV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwp_locked_pointer_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwp_locked_pointer_v1_interface }
        }
    }
    impl ZwpLockedPointerV1 {
        #[doc = "lock activation event\n\nNotification that the pointer lock of the seat's pointer is activated."]
        pub fn locked(&self) -> () {
            let msg = Event::Locked;
            self.0.send(msg);
        }
        #[doc = "lock deactivation event\n\nNotification that the pointer lock of the seat's pointer is no longer\nactive. If this is a oneshot pointer lock (see\nwp_pointer_constraints.lifetime) this object is now defunct and should\nbe destroyed. If this is a persistent pointer lock (see\nwp_pointer_constraints.lifetime) this pointer lock may again\nreactivate in the future."]
        pub fn unlocked(&self) -> () {
            let msg = Event::Unlocked;
            self.0.send(msg);
        }
    }
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_SET_CURSOR_POSITION_HINT_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_SET_REGION_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_LOCKED_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_UNLOCKED_SINCE: u32 = 1u32;
    static mut zwp_locked_pointer_v1_requests_set_region_types: [*const wl_interface; 1] =
        [unsafe { &super::wl_region::wl_region_interface as *const wl_interface }];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_locked_pointer_v1_requests: [wl_message; 3] = [
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"set_cursor_position_hint\0" as *const u8 as *const c_char,
            signature: b"ff\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"set_region\0" as *const u8 as *const c_char,
            signature: b"?o\0" as *const u8 as *const c_char,
            types: unsafe { &zwp_locked_pointer_v1_requests_set_region_types as *const _ },
        },
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_locked_pointer_v1_events: [wl_message; 2] = [
        wl_message {
            name: b"locked\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"unlocked\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwp_locked_pointer_v1_interface: wl_interface = wl_interface {
        name: b"zwp_locked_pointer_v1\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 3,
        requests: unsafe { &zwp_locked_pointer_v1_requests as *const _ },
        event_count: 2,
        events: unsafe { &zwp_locked_pointer_v1_events as *const _ },
    };
}
#[doc = "confined pointer object\n\nThe wp_confined_pointer interface represents a confined pointer state.\n\nThis object will send the event 'confined' when the confinement is\nactivated. Whenever the confinement is activated, it is guaranteed that\nthe surface the pointer is confined to will already have received pointer\nfocus and that the pointer will be within the region passed to the request\ncreating this object. It is up to the compositor to decide whether this\nrequires some user interaction and if the pointer will warp to within the\npassed region if outside.\n\nTo unconfine the pointer, send the destroy request. This will also destroy\nthe wp_confined_pointer object.\n\nIf the compositor decides to unconfine the pointer the unconfined event is\nsent. The wp_confined_pointer object is at this point defunct and should\nbe destroyed."]
pub mod zwp_confined_pointer_v1 {
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
        #[doc = "destroy the confined pointer object\n\nDestroy the confined pointer object. If applicable, the compositor will\nunconfine the pointer.\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
        #[doc = "set a new confine region\n\nSet a new region used to confine the pointer.\n\nThe new confine region is double-buffered. The new confine region will\nonly take effect when the associated surface gets its pending state\napplied. See wl_surface.commit for details.\n\nIf the confinement is active when the new confinement region is applied\nand the pointer ends up outside of newly applied region, the pointer may\nwarped to a position within the new confinement region. If warped, a\nwl_pointer.motion event will be emitted, but no\nwp_relative_pointer.relative_motion event.\n\nThe compositor may also, instead of using the new region, unconfine the\npointer.\n\nFor details about the confine region, see wp_confined_pointer."]
        SetRegion {
            region: Option<super::wl_region::WlRegion>,
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
                name: "set_region",
                since: 1,
                signature: &[super::ArgumentType::Object],
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
                Request::SetRegion { .. } => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::Destroy => 1,
                Request::SetRegion { .. } => 1,
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
                1 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::SetRegion {
                        region: {
                            if let Some(Argument::Object(val)) = args.next() {
                                if val == 0 {
                                    None
                                } else {
                                    Some(map.get(val).ok_or(())?.into())
                                }
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
                    let _args = ::std::slice::from_raw_parts(args, 1);
                    Ok(Request::SetRegion {
                        region: if _args[0].o.is_null() {
                            None
                        } else {
                            Some(
                                Resource::<super::wl_region::WlRegion>::from_c_ptr(
                                    _args[0].o as *mut _,
                                )
                                .into(),
                            )
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
    pub enum Event {
        #[doc = "pointer confined\n\nNotification that the pointer confinement of the seat's pointer is\nactivated."]
        Confined,
        #[doc = "pointer unconfined\n\nNotification that the pointer confinement of the seat's pointer is no\nlonger active. If this is a oneshot pointer confinement (see\nwp_pointer_constraints.lifetime) this object is now defunct and should\nbe destroyed. If this is a persistent pointer confinement (see\nwp_pointer_constraints.lifetime) this pointer confinement may again\nreactivate in the future."]
        Unconfined,
    }
    impl super::MessageGroup for Event {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "confined",
                since: 1,
                signature: &[],
                destructor: false,
            },
            super::MessageDesc {
                name: "unconfined",
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
                Event::Confined => 0,
                Event::Unconfined => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Event::Confined => 1,
                Event::Unconfined => 1,
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
                Event::Confined => Message {
                    sender_id: sender_id,
                    opcode: 0,
                    args: smallvec![],
                },
                Event::Unconfined => Message {
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
                Event::Confined => {
                    let mut _args_array: [wl_argument; 0] = unsafe { ::std::mem::zeroed() };
                    f(0, &mut _args_array)
                }
                Event::Unconfined => {
                    let mut _args_array: [wl_argument; 0] = unsafe { ::std::mem::zeroed() };
                    f(1, &mut _args_array)
                }
            }
        }
    }
    #[derive(Clone, Eq, PartialEq)]
    pub struct ZwpConfinedPointerV1(Resource<ZwpConfinedPointerV1>);
    impl AsRef<Resource<ZwpConfinedPointerV1>> for ZwpConfinedPointerV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwpConfinedPointerV1>> for ZwpConfinedPointerV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwpConfinedPointerV1(value)
        }
    }
    impl From<ZwpConfinedPointerV1> for Resource<ZwpConfinedPointerV1> {
        #[inline]
        fn from(value: ZwpConfinedPointerV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwpConfinedPointerV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwpConfinedPointerV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwp_confined_pointer_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwp_confined_pointer_v1_interface }
        }
    }
    impl ZwpConfinedPointerV1 {
        #[doc = "pointer confined\n\nNotification that the pointer confinement of the seat's pointer is\nactivated."]
        pub fn confined(&self) -> () {
            let msg = Event::Confined;
            self.0.send(msg);
        }
        #[doc = "pointer unconfined\n\nNotification that the pointer confinement of the seat's pointer is no\nlonger active. If this is a oneshot pointer confinement (see\nwp_pointer_constraints.lifetime) this object is now defunct and should\nbe destroyed. If this is a persistent pointer confinement (see\nwp_pointer_constraints.lifetime) this pointer confinement may again\nreactivate in the future."]
        pub fn unconfined(&self) -> () {
            let msg = Event::Unconfined;
            self.0.send(msg);
        }
    }
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_SET_REGION_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_CONFINED_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_UNCONFINED_SINCE: u32 = 1u32;
    static mut zwp_confined_pointer_v1_requests_set_region_types: [*const wl_interface; 1] =
        [unsafe { &super::wl_region::wl_region_interface as *const wl_interface }];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_confined_pointer_v1_requests: [wl_message; 2] = [
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"set_region\0" as *const u8 as *const c_char,
            signature: b"?o\0" as *const u8 as *const c_char,
            types: unsafe { &zwp_confined_pointer_v1_requests_set_region_types as *const _ },
        },
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_confined_pointer_v1_events: [wl_message; 2] = [
        wl_message {
            name: b"confined\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"unconfined\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwp_confined_pointer_v1_interface: wl_interface = wl_interface {
        name: b"zwp_confined_pointer_v1\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 2,
        requests: unsafe { &zwp_confined_pointer_v1_requests as *const _ },
        event_count: 2,
        events: unsafe { &zwp_confined_pointer_v1_events as *const _ },
    };
}
