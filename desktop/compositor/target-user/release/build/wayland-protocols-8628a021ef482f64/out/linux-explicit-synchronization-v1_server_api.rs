use std::os::raw::{c_char, c_void};
const NULLPTR: *const c_void = 0 as *const c_void;
static mut types_null: [*const sys::common::wl_interface; 1] =
    [NULLPTR as *const sys::common::wl_interface];
#[doc = "protocol for providing explicit synchronization\n\nThis global is a factory interface, allowing clients to request\nexplicit synchronization for buffers on a per-surface basis.\n\nSee zwp_linux_surface_synchronization_v1 for more information.\n\nThis interface is derived from Chromium's\nzcr_linux_explicit_synchronization_v1.\n\nWarning! The protocol described in this file is experimental and\nbackward incompatible changes may be made. Backward compatible changes\nmay be added together with the corresponding interface version bump.\nBackward incompatible changes are done by bumping the version number in\nthe protocol and interface names and resetting the interface version.\nOnce the protocol is to be declared stable, the 'z' prefix and the\nversion number in the protocol and interface names are removed and the\ninterface version number is reset."]
pub mod zwp_linux_explicit_synchronization_v1 {
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
        #[doc = "the surface already has a synchronization object associated"]
        SynchronizationExists = 0,
    }
    impl Error {
        pub fn from_raw(n: u32) -> Option<Error> {
            match n {
                0 => Some(Error::SynchronizationExists),
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
        #[doc = "destroy explicit synchronization factory object\n\nDestroy this explicit synchronization factory object. Other objects,\nincluding zwp_linux_surface_synchronization_v1 objects created by this\nfactory, shall not be affected by this request.\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
        #[doc = "extend surface interface for explicit synchronization\n\nInstantiate an interface extension for the given wl_surface to provide\nexplicit synchronization.\n\nIf the given wl_surface already has an explicit synchronization object\nassociated, the synchronization_exists protocol error is raised.\n\nGraphics APIs, like EGL or Vulkan, that manage the buffer queue and\ncommits of a wl_surface themselves, are likely to be using this\nextension internally. If a client is using such an API for a\nwl_surface, it should not directly use this extension on that surface,\nto avoid raising a synchronization_exists protocol error."]
        GetSynchronization {
            id: Main<super::zwp_linux_surface_synchronization_v1::ZwpLinuxSurfaceSynchronizationV1>,
            surface: super::wl_surface::WlSurface,
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
                name: "get_synchronization",
                since: 1,
                signature: &[super::ArgumentType::NewId, super::ArgumentType::Object],
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
                Request::GetSynchronization { .. } => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::Destroy => 1,
                Request::GetSynchronization { .. } => 1,
            }
        }
        fn child<Meta: ObjectMetadata>(
            opcode: u16,
            version: u32,
            meta: &Meta,
        ) -> Option<Object<Meta>> {
            match opcode {
                1 => Some(Object::from_interface::<
                    super::zwp_linux_surface_synchronization_v1::ZwpLinuxSurfaceSynchronizationV1,
                >(version, meta.child())),
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            match msg.opcode {
                0 => Ok(Request::Destroy),
                1 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::GetSynchronization {
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
                    Ok(Request::GetSynchronization {
                        id: {
                            let me = Resource::<ZwpLinuxExplicitSynchronizationV1>::from_c_ptr(
                                obj as *mut _,
                            );
                            me . make_child_for :: < super :: zwp_linux_surface_synchronization_v1 :: ZwpLinuxSurfaceSynchronizationV1 > (_args [0] . n) . unwrap ()
                        },
                        surface: Resource::<super::wl_surface::WlSurface>::from_c_ptr(
                            _args[1].o as *mut _,
                        )
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
    pub struct ZwpLinuxExplicitSynchronizationV1(Resource<ZwpLinuxExplicitSynchronizationV1>);
    impl AsRef<Resource<ZwpLinuxExplicitSynchronizationV1>> for ZwpLinuxExplicitSynchronizationV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwpLinuxExplicitSynchronizationV1>> for ZwpLinuxExplicitSynchronizationV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwpLinuxExplicitSynchronizationV1(value)
        }
    }
    impl From<ZwpLinuxExplicitSynchronizationV1> for Resource<ZwpLinuxExplicitSynchronizationV1> {
        #[inline]
        fn from(value: ZwpLinuxExplicitSynchronizationV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwpLinuxExplicitSynchronizationV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwpLinuxExplicitSynchronizationV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwp_linux_explicit_synchronization_v1";
        const VERSION: u32 = 2;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwp_linux_explicit_synchronization_v1_interface }
        }
    }
    impl ZwpLinuxExplicitSynchronizationV1 {}
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_GET_SYNCHRONIZATION_SINCE: u32 = 1u32;
    static mut zwp_linux_explicit_synchronization_v1_requests_get_synchronization_types:
        [*const wl_interface; 2] = [
        unsafe {
            & super :: zwp_linux_surface_synchronization_v1 :: zwp_linux_surface_synchronization_v1_interface as * const wl_interface
        },
        unsafe { &super::wl_surface::wl_surface_interface as *const wl_interface },
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_linux_explicit_synchronization_v1_requests: [wl_message; 2] = [
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"get_synchronization\0" as *const u8 as *const c_char,
            signature: b"no\0" as *const u8 as *const c_char,
            types: unsafe {
                &zwp_linux_explicit_synchronization_v1_requests_get_synchronization_types
                    as *const _
            },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwp_linux_explicit_synchronization_v1_interface: wl_interface = wl_interface {
        name: b"zwp_linux_explicit_synchronization_v1\0" as *const u8 as *const c_char,
        version: 2,
        request_count: 2,
        requests: unsafe { &zwp_linux_explicit_synchronization_v1_requests as *const _ },
        event_count: 0,
        events: NULLPTR as *const wl_message,
    };
}
#[doc = "per-surface explicit synchronization support\n\nThis object implements per-surface explicit synchronization.\n\nSynchronization refers to co-ordination of pipelined operations performed\non buffers. Most GPU clients will schedule an asynchronous operation to\nrender to the buffer, then immediately send the buffer to the compositor\nto be attached to a surface.\n\nIn implicit synchronization, ensuring that the rendering operation is\ncomplete before the compositor displays the buffer is an implementation\ndetail handled by either the kernel or userspace graphics driver.\n\nBy contrast, in explicit synchronization, dma_fence objects mark when the\nasynchronous operations are complete. When submitting a buffer, the\nclient provides an acquire fence which will be waited on before the\ncompositor accesses the buffer. The Wayland server, through a\nzwp_linux_buffer_release_v1 object, will inform the client with an event\nwhich may be accompanied by a release fence, when the compositor will no\nlonger access the buffer contents due to the specific commit that\nrequested the release event.\n\nEach surface can be associated with only one object of this interface at\nany time.\n\nIn version 1 of this interface, explicit synchronization is only\nguaranteed to be supported for buffers created with any version of the\nwp_linux_dmabuf buffer factory. Version 2 additionally guarantees\nexplicit synchronization support for opaque EGL buffers, which is a type\nof platform specific buffers described in the EGL_WL_bind_wayland_display\nextension. Compositors are free to support explicit synchronization for\nadditional buffer types."]
pub mod zwp_linux_surface_synchronization_v1 {
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
        #[doc = "the fence specified by the client could not be imported"]
        InvalidFence = 0,
        #[doc = "multiple fences added for a single surface commit"]
        DuplicateFence = 1,
        #[doc = "multiple releases added for a single surface commit"]
        DuplicateRelease = 2,
        #[doc = "the associated wl_surface was destroyed"]
        NoSurface = 3,
        #[doc = "the buffer does not support explicit synchronization"]
        UnsupportedBuffer = 4,
        #[doc = "no buffer was attached"]
        NoBuffer = 5,
    }
    impl Error {
        pub fn from_raw(n: u32) -> Option<Error> {
            match n {
                0 => Some(Error::InvalidFence),
                1 => Some(Error::DuplicateFence),
                2 => Some(Error::DuplicateRelease),
                3 => Some(Error::NoSurface),
                4 => Some(Error::UnsupportedBuffer),
                5 => Some(Error::NoBuffer),
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
        #[doc = "destroy synchronization object\n\nDestroy this explicit synchronization object.\n\nAny fence set by this object with set_acquire_fence since the last\ncommit will be discarded by the server. Any fences set by this object\nbefore the last commit are not affected.\n\nzwp_linux_buffer_release_v1 objects created by this object are not\naffected by this request.\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
        #[doc = "set the acquire fence\n\nSet the acquire fence that must be signaled before the compositor\nmay sample from the buffer attached with wl_surface.attach. The fence\nis a dma_fence kernel object.\n\nThe acquire fence is double-buffered state, and will be applied on the\nnext wl_surface.commit request for the associated surface. Thus, it\napplies only to the buffer that is attached to the surface at commit\ntime.\n\nIf the provided fd is not a valid dma_fence fd, then an INVALID_FENCE\nerror is raised.\n\nIf a fence has already been attached during the same commit cycle, a\nDUPLICATE_FENCE error is raised.\n\nIf the associated wl_surface was destroyed, a NO_SURFACE error is\nraised.\n\nIf at surface commit time the attached buffer does not support explicit\nsynchronization, an UNSUPPORTED_BUFFER error is raised.\n\nIf at surface commit time there is no buffer attached, a NO_BUFFER\nerror is raised."]
        SetAcquireFence { fd: ::std::os::unix::io::RawFd },
        #[doc = "release fence for last-attached buffer\n\nCreate a listener for the release of the buffer attached by the\nclient with wl_surface.attach. See zwp_linux_buffer_release_v1\ndocumentation for more information.\n\nThe release object is double-buffered state, and will be associated\nwith the buffer that is attached to the surface at wl_surface.commit\ntime.\n\nIf a zwp_linux_buffer_release_v1 object has already been requested for\nthe surface in the same commit cycle, a DUPLICATE_RELEASE error is\nraised.\n\nIf the associated wl_surface was destroyed, a NO_SURFACE error\nis raised.\n\nIf at surface commit time there is no buffer attached, a NO_BUFFER\nerror is raised."]
        GetRelease {
            release: Main<super::zwp_linux_buffer_release_v1::ZwpLinuxBufferReleaseV1>,
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
                name: "set_acquire_fence",
                since: 1,
                signature: &[super::ArgumentType::Fd],
                destructor: false,
            },
            super::MessageDesc {
                name: "get_release",
                since: 1,
                signature: &[super::ArgumentType::NewId],
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
                Request::SetAcquireFence { .. } => 1,
                Request::GetRelease { .. } => 2,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::Destroy => 1,
                Request::SetAcquireFence { .. } => 1,
                Request::GetRelease { .. } => 1,
            }
        }
        fn child<Meta: ObjectMetadata>(
            opcode: u16,
            version: u32,
            meta: &Meta,
        ) -> Option<Object<Meta>> {
            match opcode {
                2 => Some(Object::from_interface::<
                    super::zwp_linux_buffer_release_v1::ZwpLinuxBufferReleaseV1,
                >(version, meta.child())),
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            match msg.opcode {
                0 => Ok(Request::Destroy),
                1 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::SetAcquireFence {
                        fd: {
                            if let Some(Argument::Fd(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                2 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::GetRelease {
                        release: {
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
                0 => Ok(Request::Destroy),
                1 => {
                    let _args = ::std::slice::from_raw_parts(args, 1);
                    Ok(Request::SetAcquireFence { fd: _args[0].h })
                }
                2 => {
                    let _args = ::std::slice::from_raw_parts(args, 1);
                    Ok(Request::GetRelease {
                        release: {
                            let me = Resource::<ZwpLinuxSurfaceSynchronizationV1>::from_c_ptr(
                                obj as *mut _,
                            );
                            me . make_child_for :: < super :: zwp_linux_buffer_release_v1 :: ZwpLinuxBufferReleaseV1 > (_args [0] . n) . unwrap ()
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
    pub struct ZwpLinuxSurfaceSynchronizationV1(Resource<ZwpLinuxSurfaceSynchronizationV1>);
    impl AsRef<Resource<ZwpLinuxSurfaceSynchronizationV1>> for ZwpLinuxSurfaceSynchronizationV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwpLinuxSurfaceSynchronizationV1>> for ZwpLinuxSurfaceSynchronizationV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwpLinuxSurfaceSynchronizationV1(value)
        }
    }
    impl From<ZwpLinuxSurfaceSynchronizationV1> for Resource<ZwpLinuxSurfaceSynchronizationV1> {
        #[inline]
        fn from(value: ZwpLinuxSurfaceSynchronizationV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwpLinuxSurfaceSynchronizationV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwpLinuxSurfaceSynchronizationV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwp_linux_surface_synchronization_v1";
        const VERSION: u32 = 2;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwp_linux_surface_synchronization_v1_interface }
        }
    }
    impl ZwpLinuxSurfaceSynchronizationV1 {}
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_SET_ACQUIRE_FENCE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_GET_RELEASE_SINCE: u32 = 1u32;
    static mut zwp_linux_surface_synchronization_v1_requests_get_release_types:
        [*const wl_interface; 1] = [unsafe {
        &super::zwp_linux_buffer_release_v1::zwp_linux_buffer_release_v1_interface
            as *const wl_interface
    }];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_linux_surface_synchronization_v1_requests: [wl_message; 3] = [
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"set_acquire_fence\0" as *const u8 as *const c_char,
            signature: b"h\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"get_release\0" as *const u8 as *const c_char,
            signature: b"n\0" as *const u8 as *const c_char,
            types: unsafe {
                &zwp_linux_surface_synchronization_v1_requests_get_release_types as *const _
            },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwp_linux_surface_synchronization_v1_interface: wl_interface = wl_interface {
        name: b"zwp_linux_surface_synchronization_v1\0" as *const u8 as *const c_char,
        version: 2,
        request_count: 3,
        requests: unsafe { &zwp_linux_surface_synchronization_v1_requests as *const _ },
        event_count: 0,
        events: NULLPTR as *const wl_message,
    };
}
#[doc = "buffer release explicit synchronization\n\nThis object is instantiated in response to a\nzwp_linux_surface_synchronization_v1.get_release request.\n\nIt provides an alternative to wl_buffer.release events, providing a\nunique release from a single wl_surface.commit request. The release event\nalso supports explicit synchronization, providing a fence FD for the\nclient to synchronize against.\n\nExactly one event, either a fenced_release or an immediate_release, will\nbe emitted for the wl_surface.commit request. The compositor can choose\nrelease by release which event it uses.\n\nThis event does not replace wl_buffer.release events; servers are still\nrequired to send those events.\n\nOnce a buffer release object has delivered a 'fenced_release' or an\n'immediate_release' event it is automatically destroyed."]
pub mod zwp_linux_buffer_release_v1 {
    use super::sys::common::{wl_argument, wl_array, wl_interface, wl_message};
    use super::sys::server::*;
    use super::{
        smallvec, types_null, AnonymousObject, Argument, ArgumentType, Interface, Main, Message,
        MessageDesc, MessageGroup, Object, ObjectMetadata, Resource, NULLPTR,
    };
    use std::os::raw::c_char;
    #[derive(Debug)]
    #[non_exhaustive]
    pub enum Request {}
    impl super::MessageGroup for Request {
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
            match msg.opcode {
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
        #[doc = "release buffer with fence\n\nSent when the compositor has finalised its usage of the associated\nbuffer for the relevant commit, providing a dma_fence which will be\nsignaled when all operations by the compositor on that buffer for that\ncommit have finished.\n\nOnce the fence has signaled, and assuming the associated buffer is not\npending release from other wl_surface.commit requests, no additional\nexplicit or implicit synchronization is required to safely reuse or\ndestroy the buffer.\n\nThis event destroys the zwp_linux_buffer_release_v1 object.\n\nThis is a destructor, once sent this object cannot be used any longer."]
        FencedRelease { fence: ::std::os::unix::io::RawFd },
        #[doc = "release buffer immediately\n\nSent when the compositor has finalised its usage of the associated\nbuffer for the relevant commit, and either performed no operations\nusing it, or has a guarantee that all its operations on that buffer for\nthat commit have finished.\n\nOnce this event is received, and assuming the associated buffer is not\npending release from other wl_surface.commit requests, no additional\nexplicit or implicit synchronization is required to safely reuse or\ndestroy the buffer.\n\nThis event destroys the zwp_linux_buffer_release_v1 object.\n\nThis is a destructor, once sent this object cannot be used any longer."]
        ImmediateRelease,
    }
    impl super::MessageGroup for Event {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "fenced_release",
                since: 1,
                signature: &[super::ArgumentType::Fd],
                destructor: true,
            },
            super::MessageDesc {
                name: "immediate_release",
                since: 1,
                signature: &[],
                destructor: true,
            },
        ];
        type Map = super::ResourceMap;
        fn is_destructor(&self) -> bool {
            match *self {
                Event::FencedRelease { .. } => true,
                Event::ImmediateRelease => true,
            }
        }
        fn opcode(&self) -> u16 {
            match *self {
                Event::FencedRelease { .. } => 0,
                Event::ImmediateRelease => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Event::FencedRelease { .. } => 1,
                Event::ImmediateRelease => 1,
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
                Event::FencedRelease { fence } => Message {
                    sender_id: sender_id,
                    opcode: 0,
                    args: smallvec![Argument::Fd(fence),],
                },
                Event::ImmediateRelease => Message {
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
                Event::FencedRelease { fence } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].h = fence;
                    f(0, &mut _args_array)
                }
                Event::ImmediateRelease => {
                    let mut _args_array: [wl_argument; 0] = unsafe { ::std::mem::zeroed() };
                    f(1, &mut _args_array)
                }
            }
        }
    }
    #[derive(Clone, Eq, PartialEq)]
    pub struct ZwpLinuxBufferReleaseV1(Resource<ZwpLinuxBufferReleaseV1>);
    impl AsRef<Resource<ZwpLinuxBufferReleaseV1>> for ZwpLinuxBufferReleaseV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwpLinuxBufferReleaseV1>> for ZwpLinuxBufferReleaseV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwpLinuxBufferReleaseV1(value)
        }
    }
    impl From<ZwpLinuxBufferReleaseV1> for Resource<ZwpLinuxBufferReleaseV1> {
        #[inline]
        fn from(value: ZwpLinuxBufferReleaseV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwpLinuxBufferReleaseV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwpLinuxBufferReleaseV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwp_linux_buffer_release_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwp_linux_buffer_release_v1_interface }
        }
    }
    impl ZwpLinuxBufferReleaseV1 {
        #[doc = "release buffer with fence\n\nSent when the compositor has finalised its usage of the associated\nbuffer for the relevant commit, providing a dma_fence which will be\nsignaled when all operations by the compositor on that buffer for that\ncommit have finished.\n\nOnce the fence has signaled, and assuming the associated buffer is not\npending release from other wl_surface.commit requests, no additional\nexplicit or implicit synchronization is required to safely reuse or\ndestroy the buffer.\n\nThis event destroys the zwp_linux_buffer_release_v1 object.\n\nThis is a destructor, you cannot send requests to this object any longer once this method is called."]
        pub fn fenced_release(&self, fence: ::std::os::unix::io::RawFd) -> () {
            let msg = Event::FencedRelease { fence: fence };
            self.0.send(msg);
        }
        #[doc = "release buffer immediately\n\nSent when the compositor has finalised its usage of the associated\nbuffer for the relevant commit, and either performed no operations\nusing it, or has a guarantee that all its operations on that buffer for\nthat commit have finished.\n\nOnce this event is received, and assuming the associated buffer is not\npending release from other wl_surface.commit requests, no additional\nexplicit or implicit synchronization is required to safely reuse or\ndestroy the buffer.\n\nThis event destroys the zwp_linux_buffer_release_v1 object.\n\nThis is a destructor, you cannot send requests to this object any longer once this method is called."]
        pub fn immediate_release(&self) -> () {
            let msg = Event::ImmediateRelease;
            self.0.send(msg);
        }
    }
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_FENCED_RELEASE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_IMMEDIATE_RELEASE_SINCE: u32 = 1u32;
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_linux_buffer_release_v1_events: [wl_message; 2] = [
        wl_message {
            name: b"fenced_release\0" as *const u8 as *const c_char,
            signature: b"h\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"immediate_release\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwp_linux_buffer_release_v1_interface: wl_interface = wl_interface {
        name: b"zwp_linux_buffer_release_v1\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 0,
        requests: NULLPTR as *const wl_message,
        event_count: 2,
        events: unsafe { &zwp_linux_buffer_release_v1_events as *const _ },
    };
}
