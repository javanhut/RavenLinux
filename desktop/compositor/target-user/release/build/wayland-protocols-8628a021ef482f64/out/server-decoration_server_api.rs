use std::os::raw::{c_char, c_void};
const NULLPTR: *const c_void = 0 as *const c_void;
static mut types_null: [*const sys::common::wl_interface; 1] =
    [NULLPTR as *const sys::common::wl_interface];
#[doc = "Server side window decoration manager\n\nThis interface allows to coordinate whether the server should create\na server-side window decoration around a wl_surface representing a\nshell surface (wl_shell_surface or similar). By announcing support\nfor this interface the server indicates that it supports server\nside decorations.\n\nUse in conjunction with zxdg_decoration_manager_v1 is undefined."]
pub mod org_kde_kwin_server_decoration_manager {
    use super::sys::common::{wl_argument, wl_array, wl_interface, wl_message};
    use super::sys::server::*;
    use super::{
        smallvec, types_null, AnonymousObject, Argument, ArgumentType, Interface, Main, Message,
        MessageDesc, MessageGroup, Object, ObjectMetadata, Resource, NULLPTR,
    };
    use std::os::raw::c_char;
    #[doc = "Possible values to use in request_mode and the event mode."]
    #[repr(u32)]
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[non_exhaustive]
    pub enum Mode {
        #[doc = "Undecorated: The surface is not decorated at all, neither server nor client-side. An example is a popup surface which should not be decorated."]
        None = 0,
        #[doc = "Client-side decoration: The decoration is part of the surface and the client."]
        Client = 1,
        #[doc = "Server-side decoration: The server embeds the surface into a decoration frame."]
        Server = 2,
    }
    impl Mode {
        pub fn from_raw(n: u32) -> Option<Mode> {
            match n {
                0 => Some(Mode::None),
                1 => Some(Mode::Client),
                2 => Some(Mode::Server),
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
        #[doc = "Create a server-side decoration object for a given surface\n\nWhen a client creates a server-side decoration object it indicates\nthat it supports the protocol. The client is supposed to tell the\nserver whether it wants server-side decorations or will provide\nclient-side decorations.\n\nIf the client does not create a server-side decoration object for\na surface the server interprets this as lack of support for this\nprotocol and considers it as client-side decorated. Nevertheless a\nclient-side decorated surface should use this protocol to indicate\nto the server that it does not want a server-side deco."]
        Create {
            id: Main<super::org_kde_kwin_server_decoration::OrgKdeKwinServerDecoration>,
            surface: super::wl_surface::WlSurface,
        },
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[super::MessageDesc {
            name: "create",
            since: 1,
            signature: &[super::ArgumentType::NewId, super::ArgumentType::Object],
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
                Request::Create { .. } => 0,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::Create { .. } => 1,
            }
        }
        fn child<Meta: ObjectMetadata>(
            opcode: u16,
            version: u32,
            meta: &Meta,
        ) -> Option<Object<Meta>> {
            match opcode {
                0 => Some(Object::from_interface::<
                    super::org_kde_kwin_server_decoration::OrgKdeKwinServerDecoration,
                >(version, meta.child())),
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            match msg.opcode {
                0 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::Create {
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
                0 => {
                    let _args = ::std::slice::from_raw_parts(args, 2);
                    Ok(Request::Create {
                        id: {
                            let me = Resource::<OrgKdeKwinServerDecorationManager>::from_c_ptr(
                                obj as *mut _,
                            );
                            me . make_child_for :: < super :: org_kde_kwin_server_decoration :: OrgKdeKwinServerDecoration > (_args [0] . n) . unwrap ()
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
    pub enum Event {
        #[doc = "The default mode used on the server\n\nThis event is emitted directly after binding the interface. It contains\nthe default mode for the decoration. When a new server decoration object\nis created this new object will be in the default mode until the first\nrequest_mode is requested.\n\nThe server may change the default mode at any time."]
        DefaultMode { mode: Mode },
    }
    impl super::MessageGroup for Event {
        const MESSAGES: &'static [super::MessageDesc] = &[super::MessageDesc {
            name: "default_mode",
            since: 1,
            signature: &[super::ArgumentType::Uint],
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
                Event::DefaultMode { .. } => 0,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Event::DefaultMode { .. } => 1,
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
                Event::DefaultMode { mode } => Message {
                    sender_id: sender_id,
                    opcode: 0,
                    args: smallvec![Argument::Uint(mode.to_raw()),],
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
                Event::DefaultMode { mode } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = mode.to_raw();
                    f(0, &mut _args_array)
                }
            }
        }
    }
    #[derive(Clone, Eq, PartialEq)]
    pub struct OrgKdeKwinServerDecorationManager(Resource<OrgKdeKwinServerDecorationManager>);
    impl AsRef<Resource<OrgKdeKwinServerDecorationManager>> for OrgKdeKwinServerDecorationManager {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<OrgKdeKwinServerDecorationManager>> for OrgKdeKwinServerDecorationManager {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            OrgKdeKwinServerDecorationManager(value)
        }
    }
    impl From<OrgKdeKwinServerDecorationManager> for Resource<OrgKdeKwinServerDecorationManager> {
        #[inline]
        fn from(value: OrgKdeKwinServerDecorationManager) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for OrgKdeKwinServerDecorationManager {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for OrgKdeKwinServerDecorationManager {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "org_kde_kwin_server_decoration_manager";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &org_kde_kwin_server_decoration_manager_interface }
        }
    }
    impl OrgKdeKwinServerDecorationManager {
        #[doc = "The default mode used on the server\n\nThis event is emitted directly after binding the interface. It contains\nthe default mode for the decoration. When a new server decoration object\nis created this new object will be in the default mode until the first\nrequest_mode is requested.\n\nThe server may change the default mode at any time."]
        pub fn default_mode(&self, mode: Mode) -> () {
            let msg = Event::DefaultMode { mode: mode };
            self.0.send(msg);
        }
    }
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_CREATE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_DEFAULT_MODE_SINCE: u32 = 1u32;
    static mut org_kde_kwin_server_decoration_manager_requests_create_types: [*const wl_interface;
        2] = [
        unsafe {
            &super::org_kde_kwin_server_decoration::org_kde_kwin_server_decoration_interface
                as *const wl_interface
        },
        unsafe { &super::wl_surface::wl_surface_interface as *const wl_interface },
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut org_kde_kwin_server_decoration_manager_requests: [wl_message; 1] =
        [wl_message {
            name: b"create\0" as *const u8 as *const c_char,
            signature: b"no\0" as *const u8 as *const c_char,
            types: unsafe {
                &org_kde_kwin_server_decoration_manager_requests_create_types as *const _
            },
        }];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut org_kde_kwin_server_decoration_manager_events: [wl_message; 1] = [wl_message {
        name: b"default_mode\0" as *const u8 as *const c_char,
        signature: b"u\0" as *const u8 as *const c_char,
        types: unsafe { &types_null as *const _ },
    }];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut org_kde_kwin_server_decoration_manager_interface: wl_interface = wl_interface {
        name: b"org_kde_kwin_server_decoration_manager\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 1,
        requests: unsafe { &org_kde_kwin_server_decoration_manager_requests as *const _ },
        event_count: 1,
        events: unsafe { &org_kde_kwin_server_decoration_manager_events as *const _ },
    };
}
pub mod org_kde_kwin_server_decoration {
    use super::sys::common::{wl_argument, wl_array, wl_interface, wl_message};
    use super::sys::server::*;
    use super::{
        smallvec, types_null, AnonymousObject, Argument, ArgumentType, Interface, Main, Message,
        MessageDesc, MessageGroup, Object, ObjectMetadata, Resource, NULLPTR,
    };
    use std::os::raw::c_char;
    #[doc = "Possible values to use in request_mode and the event mode."]
    #[repr(u32)]
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[non_exhaustive]
    pub enum Mode {
        #[doc = "Undecorated: The surface is not decorated at all, neither server nor client-side. An example is a popup surface which should not be decorated."]
        None = 0,
        #[doc = "Client-side decoration: The decoration is part of the surface and the client."]
        Client = 1,
        #[doc = "Server-side decoration: The server embeds the surface into a decoration frame."]
        Server = 2,
    }
    impl Mode {
        pub fn from_raw(n: u32) -> Option<Mode> {
            match n {
                0 => Some(Mode::None),
                1 => Some(Mode::Client),
                2 => Some(Mode::Server),
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
        #[doc = "release the server decoration object\n\n\n\nThis is a destructor, once received this object cannot be used any longer."]
        Release,
        #[doc = "The decoration mode the surface wants to use."]
        RequestMode { mode: Mode },
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "release",
                since: 1,
                signature: &[],
                destructor: true,
            },
            super::MessageDesc {
                name: "request_mode",
                since: 1,
                signature: &[super::ArgumentType::Uint],
                destructor: false,
            },
        ];
        type Map = super::ResourceMap;
        fn is_destructor(&self) -> bool {
            match *self {
                Request::Release => true,
                _ => false,
            }
        }
        fn opcode(&self) -> u16 {
            match *self {
                Request::Release => 0,
                Request::RequestMode { .. } => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::Release => 1,
                Request::RequestMode { .. } => 1,
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
                0 => Ok(Request::Release),
                1 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::RequestMode {
                        mode: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                Mode::from_raw(val).ok_or(())?
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
                0 => Ok(Request::Release),
                1 => {
                    let _args = ::std::slice::from_raw_parts(args, 1);
                    Ok(Request::RequestMode {
                        mode: Mode::from_raw(_args[0].u).ok_or(())?,
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
        #[doc = "The new decoration mode applied by the server\n\nThis event is emitted directly after the decoration is created and\nrepresents the base decoration policy by the server. E.g. a server\nwhich wants all surfaces to be client-side decorated will send Client,\na server which wants server-side decoration will send Server.\n\nThe client can request a different mode through the decoration request.\nThe server will acknowledge this by another event with the same mode. So\neven if a server prefers server-side decoration it's possible to force a\nclient-side decoration.\n\nThe server may emit this event at any time. In this case the client can\nagain request a different mode. It's the responsibility of the server to\nprevent a feedback loop."]
        Mode { mode: Mode },
    }
    impl super::MessageGroup for Event {
        const MESSAGES: &'static [super::MessageDesc] = &[super::MessageDesc {
            name: "mode",
            since: 1,
            signature: &[super::ArgumentType::Uint],
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
                Event::Mode { .. } => 0,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Event::Mode { .. } => 1,
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
                Event::Mode { mode } => Message {
                    sender_id: sender_id,
                    opcode: 0,
                    args: smallvec![Argument::Uint(mode.to_raw()),],
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
                Event::Mode { mode } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = mode.to_raw();
                    f(0, &mut _args_array)
                }
            }
        }
    }
    #[derive(Clone, Eq, PartialEq)]
    pub struct OrgKdeKwinServerDecoration(Resource<OrgKdeKwinServerDecoration>);
    impl AsRef<Resource<OrgKdeKwinServerDecoration>> for OrgKdeKwinServerDecoration {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<OrgKdeKwinServerDecoration>> for OrgKdeKwinServerDecoration {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            OrgKdeKwinServerDecoration(value)
        }
    }
    impl From<OrgKdeKwinServerDecoration> for Resource<OrgKdeKwinServerDecoration> {
        #[inline]
        fn from(value: OrgKdeKwinServerDecoration) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for OrgKdeKwinServerDecoration {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for OrgKdeKwinServerDecoration {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "org_kde_kwin_server_decoration";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &org_kde_kwin_server_decoration_interface }
        }
    }
    impl OrgKdeKwinServerDecoration {
        #[doc = "The new decoration mode applied by the server\n\nThis event is emitted directly after the decoration is created and\nrepresents the base decoration policy by the server. E.g. a server\nwhich wants all surfaces to be client-side decorated will send Client,\na server which wants server-side decoration will send Server.\n\nThe client can request a different mode through the decoration request.\nThe server will acknowledge this by another event with the same mode. So\neven if a server prefers server-side decoration it's possible to force a\nclient-side decoration.\n\nThe server may emit this event at any time. In this case the client can\nagain request a different mode. It's the responsibility of the server to\nprevent a feedback loop."]
        pub fn mode(&self, mode: Mode) -> () {
            let msg = Event::Mode { mode: mode };
            self.0.send(msg);
        }
    }
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_RELEASE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_REQUEST_MODE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_MODE_SINCE: u32 = 1u32;
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut org_kde_kwin_server_decoration_requests: [wl_message; 2] = [
        wl_message {
            name: b"release\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"request_mode\0" as *const u8 as *const c_char,
            signature: b"u\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut org_kde_kwin_server_decoration_events: [wl_message; 1] = [wl_message {
        name: b"mode\0" as *const u8 as *const c_char,
        signature: b"u\0" as *const u8 as *const c_char,
        types: unsafe { &types_null as *const _ },
    }];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut org_kde_kwin_server_decoration_interface: wl_interface = wl_interface {
        name: b"org_kde_kwin_server_decoration\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 2,
        requests: unsafe { &org_kde_kwin_server_decoration_requests as *const _ },
        event_count: 1,
        events: unsafe { &org_kde_kwin_server_decoration_events as *const _ },
    };
}
