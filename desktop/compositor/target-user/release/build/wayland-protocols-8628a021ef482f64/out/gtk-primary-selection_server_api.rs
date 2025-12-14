use std::os::raw::{c_char, c_void};
const NULLPTR: *const c_void = 0 as *const c_void;
static mut types_null: [*const sys::common::wl_interface; 2] = [
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
];
#[doc = "X primary selection emulation\n\nThe primary selection device manager is a singleton global object that\nprovides access to the primary selection. It allows to create\nwp_primary_selection_source objects, as well as retrieving the per-seat\nwp_primary_selection_device objects."]
pub mod gtk_primary_selection_device_manager {
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
        #[doc = "create a new primary selection source\n\nCreate a new primary selection source."]
        CreateSource {
            id: Main<super::gtk_primary_selection_source::GtkPrimarySelectionSource>,
        },
        #[doc = "create a new primary selection device\n\nCreate a new data device for a given seat."]
        GetDevice {
            id: Main<super::gtk_primary_selection_device::GtkPrimarySelectionDevice>,
            seat: super::wl_seat::WlSeat,
        },
        #[doc = "destroy the primary selection device manager\n\nDestroy the primary selection device manager.\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "create_source",
                since: 1,
                signature: &[super::ArgumentType::NewId],
                destructor: false,
            },
            super::MessageDesc {
                name: "get_device",
                since: 1,
                signature: &[super::ArgumentType::NewId, super::ArgumentType::Object],
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
                Request::CreateSource { .. } => 0,
                Request::GetDevice { .. } => 1,
                Request::Destroy => 2,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::CreateSource { .. } => 1,
                Request::GetDevice { .. } => 1,
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
                    super::gtk_primary_selection_source::GtkPrimarySelectionSource,
                >(version, meta.child())),
                1 => Some(Object::from_interface::<
                    super::gtk_primary_selection_device::GtkPrimarySelectionDevice,
                >(version, meta.child())),
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            match msg.opcode {
                0 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::CreateSource {
                        id: {
                            if let Some(Argument::NewId(val)) = args.next() {
                                map.get_new(val).ok_or(())?
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                1 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::GetDevice {
                        id: {
                            if let Some(Argument::NewId(val)) = args.next() {
                                map.get_new(val).ok_or(())?
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
                2 => Ok(Request::Destroy),
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
                    Ok(Request::CreateSource {
                        id: {
                            let me = Resource::<GtkPrimarySelectionDeviceManager>::from_c_ptr(
                                obj as *mut _,
                            );
                            me . make_child_for :: < super :: gtk_primary_selection_source :: GtkPrimarySelectionSource > (_args [0] . n) . unwrap ()
                        },
                    })
                }
                1 => {
                    let _args = ::std::slice::from_raw_parts(args, 2);
                    Ok(Request::GetDevice {
                        id: {
                            let me = Resource::<GtkPrimarySelectionDeviceManager>::from_c_ptr(
                                obj as *mut _,
                            );
                            me . make_child_for :: < super :: gtk_primary_selection_device :: GtkPrimarySelectionDevice > (_args [0] . n) . unwrap ()
                        },
                        seat: Resource::<super::wl_seat::WlSeat>::from_c_ptr(_args[1].o as *mut _)
                            .into(),
                    })
                }
                2 => Ok(Request::Destroy),
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
    pub struct GtkPrimarySelectionDeviceManager(Resource<GtkPrimarySelectionDeviceManager>);
    impl AsRef<Resource<GtkPrimarySelectionDeviceManager>> for GtkPrimarySelectionDeviceManager {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<GtkPrimarySelectionDeviceManager>> for GtkPrimarySelectionDeviceManager {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            GtkPrimarySelectionDeviceManager(value)
        }
    }
    impl From<GtkPrimarySelectionDeviceManager> for Resource<GtkPrimarySelectionDeviceManager> {
        #[inline]
        fn from(value: GtkPrimarySelectionDeviceManager) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for GtkPrimarySelectionDeviceManager {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for GtkPrimarySelectionDeviceManager {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "gtk_primary_selection_device_manager";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &gtk_primary_selection_device_manager_interface }
        }
    }
    impl GtkPrimarySelectionDeviceManager {}
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_CREATE_SOURCE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_GET_DEVICE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    static mut gtk_primary_selection_device_manager_requests_create_source_types:
        [*const wl_interface; 1] = [unsafe {
        &super::gtk_primary_selection_source::gtk_primary_selection_source_interface
            as *const wl_interface
    }];
    static mut gtk_primary_selection_device_manager_requests_get_device_types:
        [*const wl_interface; 2] = [
        unsafe {
            &super::gtk_primary_selection_device::gtk_primary_selection_device_interface
                as *const wl_interface
        },
        unsafe { &super::wl_seat::wl_seat_interface as *const wl_interface },
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut gtk_primary_selection_device_manager_requests: [wl_message; 3] = [
        wl_message {
            name: b"create_source\0" as *const u8 as *const c_char,
            signature: b"n\0" as *const u8 as *const c_char,
            types: unsafe {
                &gtk_primary_selection_device_manager_requests_create_source_types as *const _
            },
        },
        wl_message {
            name: b"get_device\0" as *const u8 as *const c_char,
            signature: b"no\0" as *const u8 as *const c_char,
            types: unsafe {
                &gtk_primary_selection_device_manager_requests_get_device_types as *const _
            },
        },
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut gtk_primary_selection_device_manager_interface: wl_interface = wl_interface {
        name: b"gtk_primary_selection_device_manager\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 3,
        requests: unsafe { &gtk_primary_selection_device_manager_requests as *const _ },
        event_count: 0,
        events: NULLPTR as *const wl_message,
    };
}
pub mod gtk_primary_selection_device {
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
        #[doc = "set the primary selection\n\nReplaces the current selection. The previous owner of the primary selection\nwill receive a wp_primary_selection_source.cancelled event.\n\nTo unset the selection, set the source to NULL."]
        SetSelection {
            source: Option<super::gtk_primary_selection_source::GtkPrimarySelectionSource>,
            serial: u32,
        },
        #[doc = "destroy the primary selection device\n\nDestroy the primary selection device.\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "set_selection",
                since: 1,
                signature: &[super::ArgumentType::Object, super::ArgumentType::Uint],
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
                Request::SetSelection { .. } => 0,
                Request::Destroy => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::SetSelection { .. } => 1,
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
                0 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::SetSelection {
                        source: {
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
                        serial: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
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
                    let _args = ::std::slice::from_raw_parts(args, 2);
                    Ok(Request::SetSelection {
                        source: if _args[0].o.is_null() {
                            None
                        } else {
                            Some(
                                Resource::<
                                    super::gtk_primary_selection_source::GtkPrimarySelectionSource,
                                >::from_c_ptr(_args[0].o as *mut _)
                                .into(),
                            )
                        },
                        serial: _args[1].u,
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
    pub enum Event {
        #[doc = "introduce a new wp_primary_selection_offer\n\nIntroduces a new wp_primary_selection_offer object that may be used\nto receive the current primary selection. Immediately following this\nevent, the new wp_primary_selection_offer object will send\nwp_primary_selection_offer.offer events to describe the offered mime\ntypes."]
        DataOffer {
            offer: Resource<super::gtk_primary_selection_offer::GtkPrimarySelectionOffer>,
        },
        #[doc = "advertise a new primary selection\n\nThe wp_primary_selection_device.selection event is sent to notify the\nclient of a new primary selection. This event is sent after the\nwp_primary_selection.data_offer event introducing this object, and after\nthe offer has announced its mimetypes through\nwp_primary_selection_offer.offer.\n\nThe data_offer is valid until a new offer or NULL is received\nor until the client loses keyboard focus. The client must destroy the\nprevious selection data_offer, if any, upon receiving this event."]
        Selection {
            id: Option<super::gtk_primary_selection_offer::GtkPrimarySelectionOffer>,
        },
    }
    impl super::MessageGroup for Event {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "data_offer",
                since: 1,
                signature: &[super::ArgumentType::NewId],
                destructor: false,
            },
            super::MessageDesc {
                name: "selection",
                since: 1,
                signature: &[super::ArgumentType::Object],
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
                Event::DataOffer { .. } => 0,
                Event::Selection { .. } => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Event::DataOffer { .. } => 1,
                Event::Selection { .. } => 1,
            }
        }
        fn child<Meta: ObjectMetadata>(
            opcode: u16,
            version: u32,
            meta: &Meta,
        ) -> Option<Object<Meta>> {
            match opcode {
                0 => Some(Object::from_interface::<
                    super::gtk_primary_selection_offer::GtkPrimarySelectionOffer,
                >(version, meta.child())),
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            panic!("Event::from_raw can not be used Server-side.")
        }
        fn into_raw(self, sender_id: u32) -> Message {
            match self {
                Event::DataOffer { offer } => Message {
                    sender_id: sender_id,
                    opcode: 0,
                    args: smallvec![Argument::NewId(offer.id()),],
                },
                Event::Selection { id } => Message {
                    sender_id: sender_id,
                    opcode: 1,
                    args: smallvec![Argument::Object(id.map(|o| o.as_ref().id()).unwrap_or(0)),],
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
                Event::DataOffer { offer } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].o = offer.c_ptr() as *mut _;
                    f(0, &mut _args_array)
                }
                Event::Selection { id } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].o = id
                        .map(|o| o.as_ref().c_ptr() as *mut _)
                        .unwrap_or(::std::ptr::null_mut());
                    f(1, &mut _args_array)
                }
            }
        }
    }
    #[derive(Clone, Eq, PartialEq)]
    pub struct GtkPrimarySelectionDevice(Resource<GtkPrimarySelectionDevice>);
    impl AsRef<Resource<GtkPrimarySelectionDevice>> for GtkPrimarySelectionDevice {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<GtkPrimarySelectionDevice>> for GtkPrimarySelectionDevice {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            GtkPrimarySelectionDevice(value)
        }
    }
    impl From<GtkPrimarySelectionDevice> for Resource<GtkPrimarySelectionDevice> {
        #[inline]
        fn from(value: GtkPrimarySelectionDevice) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for GtkPrimarySelectionDevice {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for GtkPrimarySelectionDevice {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "gtk_primary_selection_device";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &gtk_primary_selection_device_interface }
        }
    }
    impl GtkPrimarySelectionDevice {
        #[doc = "introduce a new wp_primary_selection_offer\n\nIntroduces a new wp_primary_selection_offer object that may be used\nto receive the current primary selection. Immediately following this\nevent, the new wp_primary_selection_offer object will send\nwp_primary_selection_offer.offer events to describe the offered mime\ntypes."]
        pub fn data_offer(
            &self,
            offer: &super::gtk_primary_selection_offer::GtkPrimarySelectionOffer,
        ) -> () {
            let msg = Event::DataOffer {
                offer: offer.as_ref().clone(),
            };
            self.0.send(msg);
        }
        #[doc = "advertise a new primary selection\n\nThe wp_primary_selection_device.selection event is sent to notify the\nclient of a new primary selection. This event is sent after the\nwp_primary_selection.data_offer event introducing this object, and after\nthe offer has announced its mimetypes through\nwp_primary_selection_offer.offer.\n\nThe data_offer is valid until a new offer or NULL is received\nor until the client loses keyboard focus. The client must destroy the\nprevious selection data_offer, if any, upon receiving this event."]
        pub fn selection(
            &self,
            id: Option<&super::gtk_primary_selection_offer::GtkPrimarySelectionOffer>,
        ) -> () {
            let msg = Event::Selection {
                id: id.map(|o| o.clone()),
            };
            self.0.send(msg);
        }
    }
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_SET_SELECTION_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_DATA_OFFER_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_SELECTION_SINCE: u32 = 1u32;
    static mut gtk_primary_selection_device_requests_set_selection_types: [*const wl_interface; 2] = [
        unsafe {
            &super::gtk_primary_selection_source::gtk_primary_selection_source_interface
                as *const wl_interface
        },
        NULLPTR as *const wl_interface,
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut gtk_primary_selection_device_requests: [wl_message; 2] = [
        wl_message {
            name: b"set_selection\0" as *const u8 as *const c_char,
            signature: b"?ou\0" as *const u8 as *const c_char,
            types: unsafe {
                &gtk_primary_selection_device_requests_set_selection_types as *const _
            },
        },
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    static mut gtk_primary_selection_device_events_data_offer_types: [*const wl_interface; 1] =
        [unsafe {
            &super::gtk_primary_selection_offer::gtk_primary_selection_offer_interface
                as *const wl_interface
        }];
    static mut gtk_primary_selection_device_events_selection_types: [*const wl_interface; 1] =
        [unsafe {
            &super::gtk_primary_selection_offer::gtk_primary_selection_offer_interface
                as *const wl_interface
        }];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut gtk_primary_selection_device_events: [wl_message; 2] = [
        wl_message {
            name: b"data_offer\0" as *const u8 as *const c_char,
            signature: b"n\0" as *const u8 as *const c_char,
            types: unsafe { &gtk_primary_selection_device_events_data_offer_types as *const _ },
        },
        wl_message {
            name: b"selection\0" as *const u8 as *const c_char,
            signature: b"?o\0" as *const u8 as *const c_char,
            types: unsafe { &gtk_primary_selection_device_events_selection_types as *const _ },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut gtk_primary_selection_device_interface: wl_interface = wl_interface {
        name: b"gtk_primary_selection_device\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 2,
        requests: unsafe { &gtk_primary_selection_device_requests as *const _ },
        event_count: 2,
        events: unsafe { &gtk_primary_selection_device_events as *const _ },
    };
}
#[doc = "offer to transfer primary selection contents\n\nA wp_primary_selection_offer represents an offer to transfer the contents\nof the primary selection clipboard to the client. Similar to\nwl_data_offer, the offer also describes the mime types that the source\nwill transferthat the\ndata can be converted to and provides the mechanisms for transferring the\ndata directly to the client."]
pub mod gtk_primary_selection_offer {
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
        #[doc = "request that the data is transferred\n\nTo transfer the contents of the primary selection clipboard, the client\nissues this request and indicates the mime type that it wants to\nreceive. The transfer happens through the passed file descriptor\n(typically created with the pipe system call). The source client writes\nthe data in the mime type representation requested and then closes the\nfile descriptor.\n\nThe receiving client reads from the read end of the pipe until EOF and\ncloses its end, at which point the transfer is complete."]
        Receive {
            mime_type: String,
            fd: ::std::os::unix::io::RawFd,
        },
        #[doc = "destroy the primary selection offer\n\nDestroy the primary selection offer.\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "receive",
                since: 1,
                signature: &[super::ArgumentType::Str, super::ArgumentType::Fd],
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
                Request::Receive { .. } => 0,
                Request::Destroy => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::Receive { .. } => 1,
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
                0 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::Receive {
                        mime_type: {
                            if let Some(Argument::Str(val)) = args.next() {
                                let s = String::from_utf8(val.into_bytes()).unwrap_or_else(|e| {
                                    String::from_utf8_lossy(&e.into_bytes()).into()
                                });
                                s
                            } else {
                                return Err(());
                            }
                        },
                        fd: {
                            if let Some(Argument::Fd(val)) = args.next() {
                                val
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
                    let _args = ::std::slice::from_raw_parts(args, 2);
                    Ok(Request::Receive {
                        mime_type: ::std::ffi::CStr::from_ptr(_args[0].s)
                            .to_string_lossy()
                            .into_owned(),
                        fd: _args[1].h,
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
    pub enum Event {
        #[doc = "advertise offered mime type\n\nSent immediately after creating announcing the wp_primary_selection_offer\nthrough wp_primary_selection_device.data_offer. One event is sent per\noffered mime type."]
        Offer { mime_type: String },
    }
    impl super::MessageGroup for Event {
        const MESSAGES: &'static [super::MessageDesc] = &[super::MessageDesc {
            name: "offer",
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
                Event::Offer { .. } => 0,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Event::Offer { .. } => 1,
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
                Event::Offer { mime_type } => Message {
                    sender_id: sender_id,
                    opcode: 0,
                    args: smallvec![Argument::Str(Box::new(unsafe {
                        ::std::ffi::CString::from_vec_unchecked(mime_type.into())
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
                Event::Offer { mime_type } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    let _arg_0 = ::std::ffi::CString::new(mime_type).unwrap();
                    _args_array[0].s = _arg_0.as_ptr();
                    f(0, &mut _args_array)
                }
            }
        }
    }
    #[derive(Clone, Eq, PartialEq)]
    pub struct GtkPrimarySelectionOffer(Resource<GtkPrimarySelectionOffer>);
    impl AsRef<Resource<GtkPrimarySelectionOffer>> for GtkPrimarySelectionOffer {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<GtkPrimarySelectionOffer>> for GtkPrimarySelectionOffer {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            GtkPrimarySelectionOffer(value)
        }
    }
    impl From<GtkPrimarySelectionOffer> for Resource<GtkPrimarySelectionOffer> {
        #[inline]
        fn from(value: GtkPrimarySelectionOffer) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for GtkPrimarySelectionOffer {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for GtkPrimarySelectionOffer {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "gtk_primary_selection_offer";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &gtk_primary_selection_offer_interface }
        }
    }
    impl GtkPrimarySelectionOffer {
        #[doc = "advertise offered mime type\n\nSent immediately after creating announcing the wp_primary_selection_offer\nthrough wp_primary_selection_device.data_offer. One event is sent per\noffered mime type."]
        pub fn offer(&self, mime_type: String) -> () {
            let msg = Event::Offer {
                mime_type: mime_type,
            };
            self.0.send(msg);
        }
    }
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_RECEIVE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_OFFER_SINCE: u32 = 1u32;
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut gtk_primary_selection_offer_requests: [wl_message; 2] = [
        wl_message {
            name: b"receive\0" as *const u8 as *const c_char,
            signature: b"sh\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut gtk_primary_selection_offer_events: [wl_message; 1] = [wl_message {
        name: b"offer\0" as *const u8 as *const c_char,
        signature: b"s\0" as *const u8 as *const c_char,
        types: unsafe { &types_null as *const _ },
    }];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut gtk_primary_selection_offer_interface: wl_interface = wl_interface {
        name: b"gtk_primary_selection_offer\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 2,
        requests: unsafe { &gtk_primary_selection_offer_requests as *const _ },
        event_count: 1,
        events: unsafe { &gtk_primary_selection_offer_events as *const _ },
    };
}
#[doc = "offer to replace the contents of the primary selection\n\nThe source side of a wp_primary_selection_offer, it provides a way to\ndescribe the offered data and respond to requests to transfer the\nrequested contents of the primary selection clipboard."]
pub mod gtk_primary_selection_source {
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
        #[doc = "add an offered mime type\n\nThis request adds a mime type to the set of mime types advertised to\ntargets. Can be called several times to offer multiple types."]
        Offer { mime_type: String },
        #[doc = "destroy the primary selection source\n\nDestroy the primary selection source.\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "offer",
                since: 1,
                signature: &[super::ArgumentType::Str],
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
                Request::Offer { .. } => 0,
                Request::Destroy => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::Offer { .. } => 1,
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
                0 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::Offer {
                        mime_type: {
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
                    let _args = ::std::slice::from_raw_parts(args, 1);
                    Ok(Request::Offer {
                        mime_type: ::std::ffi::CStr::from_ptr(_args[0].s)
                            .to_string_lossy()
                            .into_owned(),
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
    pub enum Event {
        #[doc = "send the primary selection contents\n\nRequest for the current primary selection contents from the client.\nSend the specified mime type over the passed file descriptor, then\nclose it."]
        Send {
            mime_type: String,
            fd: ::std::os::unix::io::RawFd,
        },
        #[doc = "request for primary selection contents was canceled\n\nThis primary selection source is no longer valid. The client should\nclean up and destroy this primary selection source."]
        Cancelled,
    }
    impl super::MessageGroup for Event {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "send",
                since: 1,
                signature: &[super::ArgumentType::Str, super::ArgumentType::Fd],
                destructor: false,
            },
            super::MessageDesc {
                name: "cancelled",
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
                Event::Send { .. } => 0,
                Event::Cancelled => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Event::Send { .. } => 1,
                Event::Cancelled => 1,
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
                Event::Send { mime_type, fd } => Message {
                    sender_id: sender_id,
                    opcode: 0,
                    args: smallvec![
                        Argument::Str(Box::new(unsafe {
                            ::std::ffi::CString::from_vec_unchecked(mime_type.into())
                        })),
                        Argument::Fd(fd),
                    ],
                },
                Event::Cancelled => Message {
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
                Event::Send { mime_type, fd } => {
                    let mut _args_array: [wl_argument; 2] = unsafe { ::std::mem::zeroed() };
                    let _arg_0 = ::std::ffi::CString::new(mime_type).unwrap();
                    _args_array[0].s = _arg_0.as_ptr();
                    _args_array[1].h = fd;
                    f(0, &mut _args_array)
                }
                Event::Cancelled => {
                    let mut _args_array: [wl_argument; 0] = unsafe { ::std::mem::zeroed() };
                    f(1, &mut _args_array)
                }
            }
        }
    }
    #[derive(Clone, Eq, PartialEq)]
    pub struct GtkPrimarySelectionSource(Resource<GtkPrimarySelectionSource>);
    impl AsRef<Resource<GtkPrimarySelectionSource>> for GtkPrimarySelectionSource {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<GtkPrimarySelectionSource>> for GtkPrimarySelectionSource {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            GtkPrimarySelectionSource(value)
        }
    }
    impl From<GtkPrimarySelectionSource> for Resource<GtkPrimarySelectionSource> {
        #[inline]
        fn from(value: GtkPrimarySelectionSource) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for GtkPrimarySelectionSource {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for GtkPrimarySelectionSource {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "gtk_primary_selection_source";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &gtk_primary_selection_source_interface }
        }
    }
    impl GtkPrimarySelectionSource {
        #[doc = "send the primary selection contents\n\nRequest for the current primary selection contents from the client.\nSend the specified mime type over the passed file descriptor, then\nclose it."]
        pub fn send(&self, mime_type: String, fd: ::std::os::unix::io::RawFd) -> () {
            let msg = Event::Send {
                mime_type: mime_type,
                fd: fd,
            };
            self.0.send(msg);
        }
        #[doc = "request for primary selection contents was canceled\n\nThis primary selection source is no longer valid. The client should\nclean up and destroy this primary selection source."]
        pub fn cancelled(&self) -> () {
            let msg = Event::Cancelled;
            self.0.send(msg);
        }
    }
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_OFFER_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_SEND_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_CANCELLED_SINCE: u32 = 1u32;
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut gtk_primary_selection_source_requests: [wl_message; 2] = [
        wl_message {
            name: b"offer\0" as *const u8 as *const c_char,
            signature: b"s\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut gtk_primary_selection_source_events: [wl_message; 2] = [
        wl_message {
            name: b"send\0" as *const u8 as *const c_char,
            signature: b"sh\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"cancelled\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut gtk_primary_selection_source_interface: wl_interface = wl_interface {
        name: b"gtk_primary_selection_source\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 2,
        requests: unsafe { &gtk_primary_selection_source_requests as *const _ },
        event_count: 2,
        events: unsafe { &gtk_primary_selection_source_events as *const _ },
    };
}
