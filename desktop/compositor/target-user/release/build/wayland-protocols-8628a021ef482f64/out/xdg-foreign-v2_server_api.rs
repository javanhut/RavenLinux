use std::os::raw::{c_char, c_void};
const NULLPTR: *const c_void = 0 as *const c_void;
static mut types_null: [*const sys::common::wl_interface; 1] =
    [NULLPTR as *const sys::common::wl_interface];
#[doc = "interface for exporting surfaces\n\nA global interface used for exporting surfaces that can later be imported\nusing xdg_importer."]
pub mod zxdg_exporter_v2 {
    use super::sys::common::{wl_argument, wl_array, wl_interface, wl_message};
    use super::sys::server::*;
    use super::{
        smallvec, types_null, AnonymousObject, Argument, ArgumentType, Interface, Main, Message,
        MessageDesc, MessageGroup, Object, ObjectMetadata, Resource, NULLPTR,
    };
    use std::os::raw::c_char;
    #[doc = "error values\n\nThese errors can be emitted in response to invalid xdg_exporter\nrequests."]
    #[repr(u32)]
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[non_exhaustive]
    pub enum Error {
        #[doc = "surface is not an xdg_toplevel"]
        InvalidSurface = 0,
    }
    impl Error {
        pub fn from_raw(n: u32) -> Option<Error> {
            match n {
                0 => Some(Error::InvalidSurface),
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
        #[doc = "destroy the xdg_exporter object\n\nNotify the compositor that the xdg_exporter object will no longer be\nused.\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
        #[doc = "export a toplevel surface\n\nThe export_toplevel request exports the passed surface so that it can later be\nimported via xdg_importer. When called, a new xdg_exported object will\nbe created and xdg_exported.handle will be sent immediately. See the\ncorresponding interface and event for details.\n\nA surface may be exported multiple times, and each exported handle may\nbe used to create an xdg_imported multiple times. Only xdg_toplevel\nequivalent surfaces may be exported, otherwise an invalid_surface\nprotocol error is sent."]
        ExportToplevel {
            id: Main<super::zxdg_exported_v2::ZxdgExportedV2>,
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
                name: "export_toplevel",
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
                Request::ExportToplevel { .. } => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::Destroy => 1,
                Request::ExportToplevel { .. } => 1,
            }
        }
        fn child<Meta: ObjectMetadata>(
            opcode: u16,
            version: u32,
            meta: &Meta,
        ) -> Option<Object<Meta>> {
            match opcode {
                1 => Some(Object::from_interface::<
                    super::zxdg_exported_v2::ZxdgExportedV2,
                >(version, meta.child())),
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            match msg.opcode {
                0 => Ok(Request::Destroy),
                1 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::ExportToplevel {
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
                    Ok(Request::ExportToplevel {
                        id: {
                            let me = Resource::<ZxdgExporterV2>::from_c_ptr(obj as *mut _);
                            me.make_child_for::<super::zxdg_exported_v2::ZxdgExportedV2>(_args[0].n)
                                .unwrap()
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
    pub struct ZxdgExporterV2(Resource<ZxdgExporterV2>);
    impl AsRef<Resource<ZxdgExporterV2>> for ZxdgExporterV2 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZxdgExporterV2>> for ZxdgExporterV2 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZxdgExporterV2(value)
        }
    }
    impl From<ZxdgExporterV2> for Resource<ZxdgExporterV2> {
        #[inline]
        fn from(value: ZxdgExporterV2) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZxdgExporterV2 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZxdgExporterV2 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zxdg_exporter_v2";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zxdg_exporter_v2_interface }
        }
    }
    impl ZxdgExporterV2 {}
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_EXPORT_TOPLEVEL_SINCE: u32 = 1u32;
    static mut zxdg_exporter_v2_requests_export_toplevel_types: [*const wl_interface; 2] = [
        unsafe { &super::zxdg_exported_v2::zxdg_exported_v2_interface as *const wl_interface },
        unsafe { &super::wl_surface::wl_surface_interface as *const wl_interface },
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zxdg_exporter_v2_requests: [wl_message; 2] = [
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"export_toplevel\0" as *const u8 as *const c_char,
            signature: b"no\0" as *const u8 as *const c_char,
            types: unsafe { &zxdg_exporter_v2_requests_export_toplevel_types as *const _ },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zxdg_exporter_v2_interface: wl_interface = wl_interface {
        name: b"zxdg_exporter_v2\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 2,
        requests: unsafe { &zxdg_exporter_v2_requests as *const _ },
        event_count: 0,
        events: NULLPTR as *const wl_message,
    };
}
#[doc = "interface for importing surfaces\n\nA global interface used for importing surfaces exported by xdg_exporter.\nWith this interface, a client can create a reference to a surface of\nanother client."]
pub mod zxdg_importer_v2 {
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
        #[doc = "destroy the xdg_importer object\n\nNotify the compositor that the xdg_importer object will no longer be\nused.\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
        #[doc = "import a toplevel surface\n\nThe import_toplevel request imports a surface from any client given a handle\nretrieved by exporting said surface using xdg_exporter.export_toplevel.\nWhen called, a new xdg_imported object will be created. This new object\nrepresents the imported surface, and the importing client can\nmanipulate its relationship using it. See xdg_imported for details."]
        ImportToplevel {
            id: Main<super::zxdg_imported_v2::ZxdgImportedV2>,
            handle: String,
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
                name: "import_toplevel",
                since: 1,
                signature: &[super::ArgumentType::NewId, super::ArgumentType::Str],
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
                Request::ImportToplevel { .. } => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::Destroy => 1,
                Request::ImportToplevel { .. } => 1,
            }
        }
        fn child<Meta: ObjectMetadata>(
            opcode: u16,
            version: u32,
            meta: &Meta,
        ) -> Option<Object<Meta>> {
            match opcode {
                1 => Some(Object::from_interface::<
                    super::zxdg_imported_v2::ZxdgImportedV2,
                >(version, meta.child())),
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            match msg.opcode {
                0 => Ok(Request::Destroy),
                1 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::ImportToplevel {
                        id: {
                            if let Some(Argument::NewId(val)) = args.next() {
                                map.get_new(val).ok_or(())?
                            } else {
                                return Err(());
                            }
                        },
                        handle: {
                            if let Some(Argument::Str(val)) = args.next() {
                                let s = String::from_utf8(val.into_bytes()).unwrap_or_else(|e| {
                                    String::from_utf8_lossy(&e.into_bytes()).into()
                                });
                                s
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
                    Ok(Request::ImportToplevel {
                        id: {
                            let me = Resource::<ZxdgImporterV2>::from_c_ptr(obj as *mut _);
                            me.make_child_for::<super::zxdg_imported_v2::ZxdgImportedV2>(_args[0].n)
                                .unwrap()
                        },
                        handle: ::std::ffi::CStr::from_ptr(_args[1].s)
                            .to_string_lossy()
                            .into_owned(),
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
    pub struct ZxdgImporterV2(Resource<ZxdgImporterV2>);
    impl AsRef<Resource<ZxdgImporterV2>> for ZxdgImporterV2 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZxdgImporterV2>> for ZxdgImporterV2 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZxdgImporterV2(value)
        }
    }
    impl From<ZxdgImporterV2> for Resource<ZxdgImporterV2> {
        #[inline]
        fn from(value: ZxdgImporterV2) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZxdgImporterV2 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZxdgImporterV2 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zxdg_importer_v2";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zxdg_importer_v2_interface }
        }
    }
    impl ZxdgImporterV2 {}
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_IMPORT_TOPLEVEL_SINCE: u32 = 1u32;
    static mut zxdg_importer_v2_requests_import_toplevel_types: [*const wl_interface; 2] = [
        unsafe { &super::zxdg_imported_v2::zxdg_imported_v2_interface as *const wl_interface },
        NULLPTR as *const wl_interface,
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zxdg_importer_v2_requests: [wl_message; 2] = [
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"import_toplevel\0" as *const u8 as *const c_char,
            signature: b"ns\0" as *const u8 as *const c_char,
            types: unsafe { &zxdg_importer_v2_requests_import_toplevel_types as *const _ },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zxdg_importer_v2_interface: wl_interface = wl_interface {
        name: b"zxdg_importer_v2\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 2,
        requests: unsafe { &zxdg_importer_v2_requests as *const _ },
        event_count: 0,
        events: NULLPTR as *const wl_message,
    };
}
#[doc = "an exported surface handle\n\nAn xdg_exported object represents an exported reference to a surface. The\nexported surface may be referenced as long as the xdg_exported object not\ndestroyed. Destroying the xdg_exported invalidates any relationship the\nimporter may have established using xdg_imported."]
pub mod zxdg_exported_v2 {
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
        #[doc = "unexport the exported surface\n\nRevoke the previously exported surface. This invalidates any\nrelationship the importer may have set up using the xdg_imported created\ngiven the handle sent via xdg_exported.handle.\n\nThis is a destructor, once received this object cannot be used any longer."]
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
        #[doc = "the exported surface handle\n\nThe handle event contains the unique handle of this exported surface\nreference. It may be shared with any client, which then can use it to\nimport the surface by calling xdg_importer.import_toplevel. A handle\nmay be used to import the surface multiple times."]
        Handle { handle: String },
    }
    impl super::MessageGroup for Event {
        const MESSAGES: &'static [super::MessageDesc] = &[super::MessageDesc {
            name: "handle",
            since: 1,
            signature: &[super::ArgumentType::Str],
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
                Event::Handle { .. } => 0,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Event::Handle { .. } => 1,
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
                Event::Handle { handle } => Message {
                    sender_id: sender_id,
                    opcode: 0,
                    args: smallvec![Argument::Str(Box::new(unsafe {
                        ::std::ffi::CString::from_vec_unchecked(handle.into())
                    })),],
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
                Event::Handle { handle } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    let _arg_0 = ::std::ffi::CString::new(handle).unwrap();
                    _args_array[0].s = _arg_0.as_ptr();
                    f(0, &mut _args_array)
                }
            }
        }
    }
    #[derive(Clone, Eq, PartialEq)]
    pub struct ZxdgExportedV2(Resource<ZxdgExportedV2>);
    impl AsRef<Resource<ZxdgExportedV2>> for ZxdgExportedV2 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZxdgExportedV2>> for ZxdgExportedV2 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZxdgExportedV2(value)
        }
    }
    impl From<ZxdgExportedV2> for Resource<ZxdgExportedV2> {
        #[inline]
        fn from(value: ZxdgExportedV2) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZxdgExportedV2 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZxdgExportedV2 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zxdg_exported_v2";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zxdg_exported_v2_interface }
        }
    }
    impl ZxdgExportedV2 {
        #[doc = "the exported surface handle\n\nThe handle event contains the unique handle of this exported surface\nreference. It may be shared with any client, which then can use it to\nimport the surface by calling xdg_importer.import_toplevel. A handle\nmay be used to import the surface multiple times."]
        pub fn handle(&self, handle: String) -> () {
            let msg = Event::Handle { handle: handle };
            self.0.send(msg);
        }
    }
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_HANDLE_SINCE: u32 = 1u32;
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zxdg_exported_v2_requests: [wl_message; 1] = [wl_message {
        name: b"destroy\0" as *const u8 as *const c_char,
        signature: b"\0" as *const u8 as *const c_char,
        types: unsafe { &types_null as *const _ },
    }];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zxdg_exported_v2_events: [wl_message; 1] = [wl_message {
        name: b"handle\0" as *const u8 as *const c_char,
        signature: b"s\0" as *const u8 as *const c_char,
        types: unsafe { &types_null as *const _ },
    }];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zxdg_exported_v2_interface: wl_interface = wl_interface {
        name: b"zxdg_exported_v2\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 1,
        requests: unsafe { &zxdg_exported_v2_requests as *const _ },
        event_count: 1,
        events: unsafe { &zxdg_exported_v2_events as *const _ },
    };
}
#[doc = "an imported surface handle\n\nAn xdg_imported object represents an imported reference to surface exported\nby some client. A client can use this interface to manipulate\nrelationships between its own surfaces and the imported surface."]
pub mod zxdg_imported_v2 {
    use super::sys::common::{wl_argument, wl_array, wl_interface, wl_message};
    use super::sys::server::*;
    use super::{
        smallvec, types_null, AnonymousObject, Argument, ArgumentType, Interface, Main, Message,
        MessageDesc, MessageGroup, Object, ObjectMetadata, Resource, NULLPTR,
    };
    use std::os::raw::c_char;
    #[doc = "error values\n\nThese errors can be emitted in response to invalid xdg_imported\nrequests."]
    #[repr(u32)]
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[non_exhaustive]
    pub enum Error {
        #[doc = "surface is not an xdg_toplevel"]
        InvalidSurface = 0,
    }
    impl Error {
        pub fn from_raw(n: u32) -> Option<Error> {
            match n {
                0 => Some(Error::InvalidSurface),
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
        #[doc = "destroy the xdg_imported object\n\nNotify the compositor that it will no longer use the xdg_imported\nobject. Any relationship that may have been set up will at this point\nbe invalidated.\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
        #[doc = "set as the parent of some surface\n\nSet the imported surface as the parent of some surface of the client.\nThe passed surface must be an xdg_toplevel equivalent, otherwise an\ninvalid_surface protocol error is sent. Calling this function sets up\na surface to surface relation with the same stacking and positioning\nsemantics as xdg_toplevel.set_parent."]
        SetParentOf {
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
                name: "set_parent_of",
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
                Request::SetParentOf { .. } => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::Destroy => 1,
                Request::SetParentOf { .. } => 1,
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
                    Ok(Request::SetParentOf {
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
                    let _args = ::std::slice::from_raw_parts(args, 1);
                    Ok(Request::SetParentOf {
                        surface: Resource::<super::wl_surface::WlSurface>::from_c_ptr(
                            _args[0].o as *mut _,
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
        #[doc = "the imported surface handle has been destroyed\n\nThe imported surface handle has been destroyed and any relationship set\nup has been invalidated. This may happen for various reasons, for\nexample if the exported surface or the exported surface handle has been\ndestroyed, if the handle used for importing was invalid."]
        Destroyed,
    }
    impl super::MessageGroup for Event {
        const MESSAGES: &'static [super::MessageDesc] = &[super::MessageDesc {
            name: "destroyed",
            since: 1,
            signature: &[],
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
                Event::Destroyed => 0,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Event::Destroyed => 1,
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
                Event::Destroyed => Message {
                    sender_id: sender_id,
                    opcode: 0,
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
                Event::Destroyed => {
                    let mut _args_array: [wl_argument; 0] = unsafe { ::std::mem::zeroed() };
                    f(0, &mut _args_array)
                }
            }
        }
    }
    #[derive(Clone, Eq, PartialEq)]
    pub struct ZxdgImportedV2(Resource<ZxdgImportedV2>);
    impl AsRef<Resource<ZxdgImportedV2>> for ZxdgImportedV2 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZxdgImportedV2>> for ZxdgImportedV2 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZxdgImportedV2(value)
        }
    }
    impl From<ZxdgImportedV2> for Resource<ZxdgImportedV2> {
        #[inline]
        fn from(value: ZxdgImportedV2) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZxdgImportedV2 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZxdgImportedV2 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zxdg_imported_v2";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zxdg_imported_v2_interface }
        }
    }
    impl ZxdgImportedV2 {
        #[doc = "the imported surface handle has been destroyed\n\nThe imported surface handle has been destroyed and any relationship set\nup has been invalidated. This may happen for various reasons, for\nexample if the exported surface or the exported surface handle has been\ndestroyed, if the handle used for importing was invalid."]
        pub fn destroyed(&self) -> () {
            let msg = Event::Destroyed;
            self.0.send(msg);
        }
    }
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_SET_PARENT_OF_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_DESTROYED_SINCE: u32 = 1u32;
    static mut zxdg_imported_v2_requests_set_parent_of_types: [*const wl_interface; 1] =
        [unsafe { &super::wl_surface::wl_surface_interface as *const wl_interface }];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zxdg_imported_v2_requests: [wl_message; 2] = [
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"set_parent_of\0" as *const u8 as *const c_char,
            signature: b"o\0" as *const u8 as *const c_char,
            types: unsafe { &zxdg_imported_v2_requests_set_parent_of_types as *const _ },
        },
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zxdg_imported_v2_events: [wl_message; 1] = [wl_message {
        name: b"destroyed\0" as *const u8 as *const c_char,
        signature: b"\0" as *const u8 as *const c_char,
        types: unsafe { &types_null as *const _ },
    }];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zxdg_imported_v2_interface: wl_interface = wl_interface {
        name: b"zxdg_imported_v2\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 2,
        requests: unsafe { &zxdg_imported_v2_requests as *const _ },
        event_count: 1,
        events: unsafe { &zxdg_imported_v2_events as *const _ },
    };
}
