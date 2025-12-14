use std::os::raw::{c_char, c_void};
const NULLPTR: *const c_void = 0 as *const c_void;
static mut types_null: [*const sys::common::wl_interface; 2] = [
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
];
#[doc = "manager to control data devices\n\nThis interface is a manager that allows creating per-seat data device\ncontrols."]
pub mod zwlr_data_control_manager_v1 {
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
        #[doc = "create a new data source\n\nCreate a new data source."]
        CreateDataSource {
            id: Main<super::zwlr_data_control_source_v1::ZwlrDataControlSourceV1>,
        },
        #[doc = "get a data device for a seat\n\nCreate a data device that can be used to manage a seat's selection."]
        GetDataDevice {
            id: Main<super::zwlr_data_control_device_v1::ZwlrDataControlDeviceV1>,
            seat: super::wl_seat::WlSeat,
        },
        #[doc = "destroy the manager\n\nAll objects created by the manager will still remain valid, until their\nappropriate destroy request has been called.\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "create_data_source",
                since: 1,
                signature: &[super::ArgumentType::NewId],
                destructor: false,
            },
            super::MessageDesc {
                name: "get_data_device",
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
                Request::CreateDataSource { .. } => 0,
                Request::GetDataDevice { .. } => 1,
                Request::Destroy => 2,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::CreateDataSource { .. } => 1,
                Request::GetDataDevice { .. } => 1,
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
                    super::zwlr_data_control_source_v1::ZwlrDataControlSourceV1,
                >(version, meta.child())),
                1 => Some(Object::from_interface::<
                    super::zwlr_data_control_device_v1::ZwlrDataControlDeviceV1,
                >(version, meta.child())),
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            match msg.opcode {
                0 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::CreateDataSource {
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
                    Ok(Request::GetDataDevice {
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
                    Ok(Request::CreateDataSource {
                        id: {
                            let me =
                                Resource::<ZwlrDataControlManagerV1>::from_c_ptr(obj as *mut _);
                            me . make_child_for :: < super :: zwlr_data_control_source_v1 :: ZwlrDataControlSourceV1 > (_args [0] . n) . unwrap ()
                        },
                    })
                }
                1 => {
                    let _args = ::std::slice::from_raw_parts(args, 2);
                    Ok(Request::GetDataDevice {
                        id: {
                            let me =
                                Resource::<ZwlrDataControlManagerV1>::from_c_ptr(obj as *mut _);
                            me . make_child_for :: < super :: zwlr_data_control_device_v1 :: ZwlrDataControlDeviceV1 > (_args [0] . n) . unwrap ()
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
    pub struct ZwlrDataControlManagerV1(Resource<ZwlrDataControlManagerV1>);
    impl AsRef<Resource<ZwlrDataControlManagerV1>> for ZwlrDataControlManagerV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwlrDataControlManagerV1>> for ZwlrDataControlManagerV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwlrDataControlManagerV1(value)
        }
    }
    impl From<ZwlrDataControlManagerV1> for Resource<ZwlrDataControlManagerV1> {
        #[inline]
        fn from(value: ZwlrDataControlManagerV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwlrDataControlManagerV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwlrDataControlManagerV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwlr_data_control_manager_v1";
        const VERSION: u32 = 2;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwlr_data_control_manager_v1_interface }
        }
    }
    impl ZwlrDataControlManagerV1 {}
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_CREATE_DATA_SOURCE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_GET_DATA_DEVICE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    static mut zwlr_data_control_manager_v1_requests_create_data_source_types:
        [*const wl_interface; 1] = [unsafe {
        &super::zwlr_data_control_source_v1::zwlr_data_control_source_v1_interface
            as *const wl_interface
    }];
    static mut zwlr_data_control_manager_v1_requests_get_data_device_types: [*const wl_interface;
        2] = [
        unsafe {
            &super::zwlr_data_control_device_v1::zwlr_data_control_device_v1_interface
                as *const wl_interface
        },
        unsafe { &super::wl_seat::wl_seat_interface as *const wl_interface },
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwlr_data_control_manager_v1_requests: [wl_message; 3] = [
        wl_message {
            name: b"create_data_source\0" as *const u8 as *const c_char,
            signature: b"n\0" as *const u8 as *const c_char,
            types: unsafe {
                &zwlr_data_control_manager_v1_requests_create_data_source_types as *const _
            },
        },
        wl_message {
            name: b"get_data_device\0" as *const u8 as *const c_char,
            signature: b"no\0" as *const u8 as *const c_char,
            types: unsafe {
                &zwlr_data_control_manager_v1_requests_get_data_device_types as *const _
            },
        },
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwlr_data_control_manager_v1_interface: wl_interface = wl_interface {
        name: b"zwlr_data_control_manager_v1\0" as *const u8 as *const c_char,
        version: 2,
        request_count: 3,
        requests: unsafe { &zwlr_data_control_manager_v1_requests as *const _ },
        event_count: 0,
        events: NULLPTR as *const wl_message,
    };
}
#[doc = "manage a data device for a seat\n\nThis interface allows a client to manage a seat's selection.\n\nWhen the seat is destroyed, this object becomes inert."]
pub mod zwlr_data_control_device_v1 {
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
        #[doc = "source given to set_selection or set_primary_selection was already used before"]
        UsedSource = 1,
    }
    impl Error {
        pub fn from_raw(n: u32) -> Option<Error> {
            match n {
                1 => Some(Error::UsedSource),
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
        #[doc = "copy data to the selection\n\nThis request asks the compositor to set the selection to the data from\nthe source on behalf of the client.\n\nThe given source may not be used in any further set_selection or\nset_primary_selection requests. Attempting to use a previously used\nsource is a protocol error.\n\nTo unset the selection, set the source to NULL."]
        SetSelection {
            source: Option<super::zwlr_data_control_source_v1::ZwlrDataControlSourceV1>,
        },
        #[doc = "destroy this data device\n\nDestroys the data device object.\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
        #[doc = "copy data to the primary selection\n\nThis request asks the compositor to set the primary selection to the\ndata from the source on behalf of the client.\n\nThe given source may not be used in any further set_selection or\nset_primary_selection requests. Attempting to use a previously used\nsource is a protocol error.\n\nTo unset the primary selection, set the source to NULL.\n\nThe compositor will ignore this request if it does not support primary\nselection.\n\nOnly available since version 2 of the interface"]
        SetPrimarySelection {
            source: Option<super::zwlr_data_control_source_v1::ZwlrDataControlSourceV1>,
        },
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "set_selection",
                since: 1,
                signature: &[super::ArgumentType::Object],
                destructor: false,
            },
            super::MessageDesc {
                name: "destroy",
                since: 1,
                signature: &[],
                destructor: true,
            },
            super::MessageDesc {
                name: "set_primary_selection",
                since: 2,
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
                Request::SetSelection { .. } => 0,
                Request::Destroy => 1,
                Request::SetPrimarySelection { .. } => 2,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::SetSelection { .. } => 1,
                Request::Destroy => 1,
                Request::SetPrimarySelection { .. } => 2,
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
                    })
                }
                1 => Ok(Request::Destroy),
                2 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::SetPrimarySelection {
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
                    Ok(Request::SetSelection {
                        source: if _args[0].o.is_null() {
                            None
                        } else {
                            Some(
                                Resource::<
                                    super::zwlr_data_control_source_v1::ZwlrDataControlSourceV1,
                                >::from_c_ptr(_args[0].o as *mut _)
                                .into(),
                            )
                        },
                    })
                }
                1 => Ok(Request::Destroy),
                2 => {
                    let _args = ::std::slice::from_raw_parts(args, 1);
                    Ok(Request::SetPrimarySelection {
                        source: if _args[0].o.is_null() {
                            None
                        } else {
                            Some(
                                Resource::<
                                    super::zwlr_data_control_source_v1::ZwlrDataControlSourceV1,
                                >::from_c_ptr(_args[0].o as *mut _)
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
        #[doc = "introduce a new wlr_data_control_offer\n\nThe data_offer event introduces a new wlr_data_control_offer object,\nwhich will subsequently be used in either the\nwlr_data_control_device.selection event (for the regular clipboard\nselections) or the wlr_data_control_device.primary_selection event (for\nthe primary clipboard selections). Immediately following the\nwlr_data_control_device.data_offer event, the new data_offer object\nwill send out wlr_data_control_offer.offer events to describe the MIME\ntypes it offers."]
        DataOffer {
            id: Resource<super::zwlr_data_control_offer_v1::ZwlrDataControlOfferV1>,
        },
        #[doc = "advertise new selection\n\nThe selection event is sent out to notify the client of a new\nwlr_data_control_offer for the selection for this device. The\nwlr_data_control_device.data_offer and the wlr_data_control_offer.offer\nevents are sent out immediately before this event to introduce the data\noffer object. The selection event is sent to a client when a new\nselection is set. The wlr_data_control_offer is valid until a new\nwlr_data_control_offer or NULL is received. The client must destroy the\nprevious selection wlr_data_control_offer, if any, upon receiving this\nevent.\n\nThe first selection event is sent upon binding the\nwlr_data_control_device object."]
        Selection {
            id: Option<super::zwlr_data_control_offer_v1::ZwlrDataControlOfferV1>,
        },
        #[doc = "this data control is no longer valid\n\nThis data control object is no longer valid and should be destroyed by\nthe client."]
        Finished,
        #[doc = "advertise new primary selection\n\nThe primary_selection event is sent out to notify the client of a new\nwlr_data_control_offer for the primary selection for this device. The\nwlr_data_control_device.data_offer and the wlr_data_control_offer.offer\nevents are sent out immediately before this event to introduce the data\noffer object. The primary_selection event is sent to a client when a\nnew primary selection is set. The wlr_data_control_offer is valid until\na new wlr_data_control_offer or NULL is received. The client must\ndestroy the previous primary selection wlr_data_control_offer, if any,\nupon receiving this event.\n\nIf the compositor supports primary selection, the first\nprimary_selection event is sent upon binding the\nwlr_data_control_device object.\n\nOnly available since version 2 of the interface"]
        PrimarySelection {
            id: Option<super::zwlr_data_control_offer_v1::ZwlrDataControlOfferV1>,
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
            super::MessageDesc {
                name: "finished",
                since: 1,
                signature: &[],
                destructor: false,
            },
            super::MessageDesc {
                name: "primary_selection",
                since: 2,
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
                Event::Finished => 2,
                Event::PrimarySelection { .. } => 3,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Event::DataOffer { .. } => 1,
                Event::Selection { .. } => 1,
                Event::Finished => 1,
                Event::PrimarySelection { .. } => 2,
            }
        }
        fn child<Meta: ObjectMetadata>(
            opcode: u16,
            version: u32,
            meta: &Meta,
        ) -> Option<Object<Meta>> {
            match opcode {
                0 => Some(Object::from_interface::<
                    super::zwlr_data_control_offer_v1::ZwlrDataControlOfferV1,
                >(version, meta.child())),
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            panic!("Event::from_raw can not be used Server-side.")
        }
        fn into_raw(self, sender_id: u32) -> Message {
            match self {
                Event::DataOffer { id } => Message {
                    sender_id: sender_id,
                    opcode: 0,
                    args: smallvec![Argument::NewId(id.id()),],
                },
                Event::Selection { id } => Message {
                    sender_id: sender_id,
                    opcode: 1,
                    args: smallvec![Argument::Object(id.map(|o| o.as_ref().id()).unwrap_or(0)),],
                },
                Event::Finished => Message {
                    sender_id: sender_id,
                    opcode: 2,
                    args: smallvec![],
                },
                Event::PrimarySelection { id } => Message {
                    sender_id: sender_id,
                    opcode: 3,
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
                Event::DataOffer { id } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].o = id.c_ptr() as *mut _;
                    f(0, &mut _args_array)
                }
                Event::Selection { id } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].o = id
                        .map(|o| o.as_ref().c_ptr() as *mut _)
                        .unwrap_or(::std::ptr::null_mut());
                    f(1, &mut _args_array)
                }
                Event::Finished => {
                    let mut _args_array: [wl_argument; 0] = unsafe { ::std::mem::zeroed() };
                    f(2, &mut _args_array)
                }
                Event::PrimarySelection { id } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].o = id
                        .map(|o| o.as_ref().c_ptr() as *mut _)
                        .unwrap_or(::std::ptr::null_mut());
                    f(3, &mut _args_array)
                }
            }
        }
    }
    #[derive(Clone, Eq, PartialEq)]
    pub struct ZwlrDataControlDeviceV1(Resource<ZwlrDataControlDeviceV1>);
    impl AsRef<Resource<ZwlrDataControlDeviceV1>> for ZwlrDataControlDeviceV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwlrDataControlDeviceV1>> for ZwlrDataControlDeviceV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwlrDataControlDeviceV1(value)
        }
    }
    impl From<ZwlrDataControlDeviceV1> for Resource<ZwlrDataControlDeviceV1> {
        #[inline]
        fn from(value: ZwlrDataControlDeviceV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwlrDataControlDeviceV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwlrDataControlDeviceV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwlr_data_control_device_v1";
        const VERSION: u32 = 2;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwlr_data_control_device_v1_interface }
        }
    }
    impl ZwlrDataControlDeviceV1 {
        #[doc = "introduce a new wlr_data_control_offer\n\nThe data_offer event introduces a new wlr_data_control_offer object,\nwhich will subsequently be used in either the\nwlr_data_control_device.selection event (for the regular clipboard\nselections) or the wlr_data_control_device.primary_selection event (for\nthe primary clipboard selections). Immediately following the\nwlr_data_control_device.data_offer event, the new data_offer object\nwill send out wlr_data_control_offer.offer events to describe the MIME\ntypes it offers."]
        pub fn data_offer(
            &self,
            id: &super::zwlr_data_control_offer_v1::ZwlrDataControlOfferV1,
        ) -> () {
            let msg = Event::DataOffer {
                id: id.as_ref().clone(),
            };
            self.0.send(msg);
        }
        #[doc = "advertise new selection\n\nThe selection event is sent out to notify the client of a new\nwlr_data_control_offer for the selection for this device. The\nwlr_data_control_device.data_offer and the wlr_data_control_offer.offer\nevents are sent out immediately before this event to introduce the data\noffer object. The selection event is sent to a client when a new\nselection is set. The wlr_data_control_offer is valid until a new\nwlr_data_control_offer or NULL is received. The client must destroy the\nprevious selection wlr_data_control_offer, if any, upon receiving this\nevent.\n\nThe first selection event is sent upon binding the\nwlr_data_control_device object."]
        pub fn selection(
            &self,
            id: Option<&super::zwlr_data_control_offer_v1::ZwlrDataControlOfferV1>,
        ) -> () {
            let msg = Event::Selection {
                id: id.map(|o| o.clone()),
            };
            self.0.send(msg);
        }
        #[doc = "this data control is no longer valid\n\nThis data control object is no longer valid and should be destroyed by\nthe client."]
        pub fn finished(&self) -> () {
            let msg = Event::Finished;
            self.0.send(msg);
        }
        #[doc = "advertise new primary selection\n\nThe primary_selection event is sent out to notify the client of a new\nwlr_data_control_offer for the primary selection for this device. The\nwlr_data_control_device.data_offer and the wlr_data_control_offer.offer\nevents are sent out immediately before this event to introduce the data\noffer object. The primary_selection event is sent to a client when a\nnew primary selection is set. The wlr_data_control_offer is valid until\na new wlr_data_control_offer or NULL is received. The client must\ndestroy the previous primary selection wlr_data_control_offer, if any,\nupon receiving this event.\n\nIf the compositor supports primary selection, the first\nprimary_selection event is sent upon binding the\nwlr_data_control_device object.\n\nOnly available since version 2 of the interface."]
        pub fn primary_selection(
            &self,
            id: Option<&super::zwlr_data_control_offer_v1::ZwlrDataControlOfferV1>,
        ) -> () {
            let msg = Event::PrimarySelection {
                id: id.map(|o| o.clone()),
            };
            self.0.send(msg);
        }
    }
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_SET_SELECTION_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_SET_PRIMARY_SELECTION_SINCE: u32 = 2u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_DATA_OFFER_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_SELECTION_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_FINISHED_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_PRIMARY_SELECTION_SINCE: u32 = 2u32;
    static mut zwlr_data_control_device_v1_requests_set_selection_types: [*const wl_interface; 1] =
        [unsafe {
            &super::zwlr_data_control_source_v1::zwlr_data_control_source_v1_interface
                as *const wl_interface
        }];
    static mut zwlr_data_control_device_v1_requests_set_primary_selection_types:
        [*const wl_interface; 1] = [unsafe {
        &super::zwlr_data_control_source_v1::zwlr_data_control_source_v1_interface
            as *const wl_interface
    }];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwlr_data_control_device_v1_requests: [wl_message; 3] = [
        wl_message {
            name: b"set_selection\0" as *const u8 as *const c_char,
            signature: b"?o\0" as *const u8 as *const c_char,
            types: unsafe { &zwlr_data_control_device_v1_requests_set_selection_types as *const _ },
        },
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"set_primary_selection\0" as *const u8 as *const c_char,
            signature: b"2?o\0" as *const u8 as *const c_char,
            types: unsafe {
                &zwlr_data_control_device_v1_requests_set_primary_selection_types as *const _
            },
        },
    ];
    static mut zwlr_data_control_device_v1_events_data_offer_types: [*const wl_interface; 1] =
        [unsafe {
            &super::zwlr_data_control_offer_v1::zwlr_data_control_offer_v1_interface
                as *const wl_interface
        }];
    static mut zwlr_data_control_device_v1_events_selection_types: [*const wl_interface; 1] =
        [unsafe {
            &super::zwlr_data_control_offer_v1::zwlr_data_control_offer_v1_interface
                as *const wl_interface
        }];
    static mut zwlr_data_control_device_v1_events_primary_selection_types: [*const wl_interface;
        1] = [unsafe {
        &super::zwlr_data_control_offer_v1::zwlr_data_control_offer_v1_interface
            as *const wl_interface
    }];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwlr_data_control_device_v1_events: [wl_message; 4] = [
        wl_message {
            name: b"data_offer\0" as *const u8 as *const c_char,
            signature: b"n\0" as *const u8 as *const c_char,
            types: unsafe { &zwlr_data_control_device_v1_events_data_offer_types as *const _ },
        },
        wl_message {
            name: b"selection\0" as *const u8 as *const c_char,
            signature: b"?o\0" as *const u8 as *const c_char,
            types: unsafe { &zwlr_data_control_device_v1_events_selection_types as *const _ },
        },
        wl_message {
            name: b"finished\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"primary_selection\0" as *const u8 as *const c_char,
            signature: b"2?o\0" as *const u8 as *const c_char,
            types: unsafe {
                &zwlr_data_control_device_v1_events_primary_selection_types as *const _
            },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwlr_data_control_device_v1_interface: wl_interface = wl_interface {
        name: b"zwlr_data_control_device_v1\0" as *const u8 as *const c_char,
        version: 2,
        request_count: 3,
        requests: unsafe { &zwlr_data_control_device_v1_requests as *const _ },
        event_count: 4,
        events: unsafe { &zwlr_data_control_device_v1_events as *const _ },
    };
}
#[doc = "offer to transfer data\n\nThe wlr_data_control_source object is the source side of a\nwlr_data_control_offer. It is created by the source client in a data\ntransfer and provides a way to describe the offered data and a way to\nrespond to requests to transfer the data."]
pub mod zwlr_data_control_source_v1 {
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
        #[doc = "offer sent after wlr_data_control_device.set_selection"]
        InvalidOffer = 1,
    }
    impl Error {
        pub fn from_raw(n: u32) -> Option<Error> {
            match n {
                1 => Some(Error::InvalidOffer),
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
        #[doc = "add an offered MIME type\n\nThis request adds a MIME type to the set of MIME types advertised to\ntargets. Can be called several times to offer multiple types.\n\nCalling this after wlr_data_control_device.set_selection is a protocol\nerror."]
        Offer { mime_type: String },
        #[doc = "destroy this source\n\nDestroys the data source object.\n\nThis is a destructor, once received this object cannot be used any longer."]
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
        #[doc = "send the data\n\nRequest for data from the client. Send the data as the specified MIME\ntype over the passed file descriptor, then close it."]
        Send {
            mime_type: String,
            fd: ::std::os::unix::io::RawFd,
        },
        #[doc = "selection was cancelled\n\nThis data source is no longer valid. The data source has been replaced\nby another data source.\n\nThe client should clean up and destroy this data source."]
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
    pub struct ZwlrDataControlSourceV1(Resource<ZwlrDataControlSourceV1>);
    impl AsRef<Resource<ZwlrDataControlSourceV1>> for ZwlrDataControlSourceV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwlrDataControlSourceV1>> for ZwlrDataControlSourceV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwlrDataControlSourceV1(value)
        }
    }
    impl From<ZwlrDataControlSourceV1> for Resource<ZwlrDataControlSourceV1> {
        #[inline]
        fn from(value: ZwlrDataControlSourceV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwlrDataControlSourceV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwlrDataControlSourceV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwlr_data_control_source_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwlr_data_control_source_v1_interface }
        }
    }
    impl ZwlrDataControlSourceV1 {
        #[doc = "send the data\n\nRequest for data from the client. Send the data as the specified MIME\ntype over the passed file descriptor, then close it."]
        pub fn send(&self, mime_type: String, fd: ::std::os::unix::io::RawFd) -> () {
            let msg = Event::Send {
                mime_type: mime_type,
                fd: fd,
            };
            self.0.send(msg);
        }
        #[doc = "selection was cancelled\n\nThis data source is no longer valid. The data source has been replaced\nby another data source.\n\nThe client should clean up and destroy this data source."]
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
    pub static mut zwlr_data_control_source_v1_requests: [wl_message; 2] = [
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
    pub static mut zwlr_data_control_source_v1_events: [wl_message; 2] = [
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
    pub static mut zwlr_data_control_source_v1_interface: wl_interface = wl_interface {
        name: b"zwlr_data_control_source_v1\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 2,
        requests: unsafe { &zwlr_data_control_source_v1_requests as *const _ },
        event_count: 2,
        events: unsafe { &zwlr_data_control_source_v1_events as *const _ },
    };
}
#[doc = "offer to transfer data\n\nA wlr_data_control_offer represents a piece of data offered for transfer\nby another client (the source client). The offer describes the different\nMIME types that the data can be converted to and provides the mechanism\nfor transferring the data directly from the source client."]
pub mod zwlr_data_control_offer_v1 {
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
        #[doc = "request that the data is transferred\n\nTo transfer the offered data, the client issues this request and\nindicates the MIME type it wants to receive. The transfer happens\nthrough the passed file descriptor (typically created with the pipe\nsystem call). The source client writes the data in the MIME type\nrepresentation requested and then closes the file descriptor.\n\nThe receiving client reads from the read end of the pipe until EOF and\nthen closes its end, at which point the transfer is complete.\n\nThis request may happen multiple times for different MIME types."]
        Receive {
            mime_type: String,
            fd: ::std::os::unix::io::RawFd,
        },
        #[doc = "destroy this offer\n\nDestroys the data offer object.\n\nThis is a destructor, once received this object cannot be used any longer."]
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
        #[doc = "advertise offered MIME type\n\nSent immediately after creating the wlr_data_control_offer object.\nOne event per offered MIME type."]
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
    pub struct ZwlrDataControlOfferV1(Resource<ZwlrDataControlOfferV1>);
    impl AsRef<Resource<ZwlrDataControlOfferV1>> for ZwlrDataControlOfferV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwlrDataControlOfferV1>> for ZwlrDataControlOfferV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwlrDataControlOfferV1(value)
        }
    }
    impl From<ZwlrDataControlOfferV1> for Resource<ZwlrDataControlOfferV1> {
        #[inline]
        fn from(value: ZwlrDataControlOfferV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwlrDataControlOfferV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwlrDataControlOfferV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwlr_data_control_offer_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwlr_data_control_offer_v1_interface }
        }
    }
    impl ZwlrDataControlOfferV1 {
        #[doc = "advertise offered MIME type\n\nSent immediately after creating the wlr_data_control_offer object.\nOne event per offered MIME type."]
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
    pub static mut zwlr_data_control_offer_v1_requests: [wl_message; 2] = [
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
    pub static mut zwlr_data_control_offer_v1_events: [wl_message; 1] = [wl_message {
        name: b"offer\0" as *const u8 as *const c_char,
        signature: b"s\0" as *const u8 as *const c_char,
        types: unsafe { &types_null as *const _ },
    }];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwlr_data_control_offer_v1_interface: wl_interface = wl_interface {
        name: b"zwlr_data_control_offer_v1\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 2,
        requests: unsafe { &zwlr_data_control_offer_v1_requests as *const _ },
        event_count: 1,
        events: unsafe { &zwlr_data_control_offer_v1_events as *const _ },
    };
}
