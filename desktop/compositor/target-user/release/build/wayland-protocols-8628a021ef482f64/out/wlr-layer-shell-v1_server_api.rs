use std::os::raw::{c_char, c_void};
const NULLPTR: *const c_void = 0 as *const c_void;
static mut types_null: [*const sys::common::wl_interface; 4] = [
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
];
#[doc = "create surfaces that are layers of the desktop\n\nClients can use this interface to assign the surface_layer role to\nwl_surfaces. Such surfaces are assigned to a \"layer\" of the output and\nrendered with a defined z-depth respective to each other. They may also be\nanchored to the edges and corners of a screen and specify input handling\nsemantics. This interface should be suitable for the implementation of\nmany desktop shell components, and a broad number of other applications\nthat interact with the desktop."]
pub mod zwlr_layer_shell_v1 {
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
        #[doc = "wl_surface has another role"]
        Role = 0,
        #[doc = "layer value is invalid"]
        InvalidLayer = 1,
        #[doc = "wl_surface has a buffer attached or committed"]
        AlreadyConstructed = 2,
    }
    impl Error {
        pub fn from_raw(n: u32) -> Option<Error> {
            match n {
                0 => Some(Error::Role),
                1 => Some(Error::InvalidLayer),
                2 => Some(Error::AlreadyConstructed),
                _ => Option::None,
            }
        }
        pub fn to_raw(&self) -> u32 {
            *self as u32
        }
    }
    #[doc = "available layers for surfaces\n\nThese values indicate which layers a surface can be rendered in. They\nare ordered by z depth, bottom-most first. Traditional shell surfaces\nwill typically be rendered between the bottom and top layers.\nFullscreen shell surfaces are typically rendered at the top layer.\nMultiple surfaces can share a single layer, and ordering within a\nsingle layer is undefined."]
    #[repr(u32)]
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[non_exhaustive]
    pub enum Layer {
        Background = 0,
        Bottom = 1,
        Top = 2,
        Overlay = 3,
    }
    impl Layer {
        pub fn from_raw(n: u32) -> Option<Layer> {
            match n {
                0 => Some(Layer::Background),
                1 => Some(Layer::Bottom),
                2 => Some(Layer::Top),
                3 => Some(Layer::Overlay),
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
        #[doc = "create a layer_surface from a surface\n\nCreate a layer surface for an existing surface. This assigns the role of\nlayer_surface, or raises a protocol error if another role is already\nassigned.\n\nCreating a layer surface from a wl_surface which has a buffer attached\nor committed is a client error, and any attempts by a client to attach\nor manipulate a buffer prior to the first layer_surface.configure call\nmust also be treated as errors.\n\nAfter creating a layer_surface object and setting it up, the client\nmust perform an initial commit without any buffer attached.\nThe compositor will reply with a layer_surface.configure event.\nThe client must acknowledge it and is then allowed to attach a buffer\nto map the surface.\n\nYou may pass NULL for output to allow the compositor to decide which\noutput to use. Generally this will be the one that the user most\nrecently interacted with.\n\nClients can specify a namespace that defines the purpose of the layer\nsurface."]
        GetLayerSurface {
            id: Main<super::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1>,
            surface: super::wl_surface::WlSurface,
            output: Option<super::wl_output::WlOutput>,
            layer: Layer,
            namespace: String,
        },
        #[doc = "destroy the layer_shell object\n\nThis request indicates that the client will not use the layer_shell\nobject any more. Objects that have been created through this instance\nare not affected.\n\nThis is a destructor, once received this object cannot be used any longer.\nOnly available since version 3 of the interface"]
        Destroy,
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "get_layer_surface",
                since: 1,
                signature: &[
                    super::ArgumentType::NewId,
                    super::ArgumentType::Object,
                    super::ArgumentType::Object,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Str,
                ],
                destructor: false,
            },
            super::MessageDesc {
                name: "destroy",
                since: 3,
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
                Request::GetLayerSurface { .. } => 0,
                Request::Destroy => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::GetLayerSurface { .. } => 1,
                Request::Destroy => 3,
            }
        }
        fn child<Meta: ObjectMetadata>(
            opcode: u16,
            version: u32,
            meta: &Meta,
        ) -> Option<Object<Meta>> {
            match opcode {
                0 => Some(Object::from_interface::<
                    super::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
                >(version, meta.child())),
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            match msg.opcode {
                0 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::GetLayerSurface {
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
                        output: {
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
                        layer: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                Layer::from_raw(val).ok_or(())?
                            } else {
                                return Err(());
                            }
                        },
                        namespace: {
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
                    let _args = ::std::slice::from_raw_parts(args, 5);
                    Ok(Request::GetLayerSurface {
                        id: {
                            let me = Resource::<ZwlrLayerShellV1>::from_c_ptr(obj as *mut _);
                            me.make_child_for::<super::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1>(
                                _args[0].n,
                            )
                            .unwrap()
                        },
                        surface: Resource::<super::wl_surface::WlSurface>::from_c_ptr(
                            _args[1].o as *mut _,
                        )
                        .into(),
                        output: if _args[2].o.is_null() {
                            None
                        } else {
                            Some(
                                Resource::<super::wl_output::WlOutput>::from_c_ptr(
                                    _args[2].o as *mut _,
                                )
                                .into(),
                            )
                        },
                        layer: Layer::from_raw(_args[3].u).ok_or(())?,
                        namespace: ::std::ffi::CStr::from_ptr(_args[4].s)
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
    pub struct ZwlrLayerShellV1(Resource<ZwlrLayerShellV1>);
    impl AsRef<Resource<ZwlrLayerShellV1>> for ZwlrLayerShellV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwlrLayerShellV1>> for ZwlrLayerShellV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwlrLayerShellV1(value)
        }
    }
    impl From<ZwlrLayerShellV1> for Resource<ZwlrLayerShellV1> {
        #[inline]
        fn from(value: ZwlrLayerShellV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwlrLayerShellV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwlrLayerShellV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwlr_layer_shell_v1";
        const VERSION: u32 = 4;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwlr_layer_shell_v1_interface }
        }
    }
    impl ZwlrLayerShellV1 {}
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_GET_LAYER_SURFACE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 3u32;
    static mut zwlr_layer_shell_v1_requests_get_layer_surface_types: [*const wl_interface; 5] = [
        unsafe {
            &super::zwlr_layer_surface_v1::zwlr_layer_surface_v1_interface as *const wl_interface
        },
        unsafe { &super::wl_surface::wl_surface_interface as *const wl_interface },
        unsafe { &super::wl_output::wl_output_interface as *const wl_interface },
        NULLPTR as *const wl_interface,
        NULLPTR as *const wl_interface,
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwlr_layer_shell_v1_requests: [wl_message; 2] = [
        wl_message {
            name: b"get_layer_surface\0" as *const u8 as *const c_char,
            signature: b"no?ous\0" as *const u8 as *const c_char,
            types: unsafe { &zwlr_layer_shell_v1_requests_get_layer_surface_types as *const _ },
        },
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"3\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwlr_layer_shell_v1_interface: wl_interface = wl_interface {
        name: b"zwlr_layer_shell_v1\0" as *const u8 as *const c_char,
        version: 4,
        request_count: 2,
        requests: unsafe { &zwlr_layer_shell_v1_requests as *const _ },
        event_count: 0,
        events: NULLPTR as *const wl_message,
    };
}
#[doc = "layer metadata interface\n\nAn interface that may be implemented by a wl_surface, for surfaces that\nare designed to be rendered as a layer of a stacked desktop-like\nenvironment.\n\nLayer surface state (layer, size, anchor, exclusive zone,\nmargin, interactivity) is double-buffered, and will be applied at the\ntime wl_surface.commit of the corresponding wl_surface is called.\n\nAttaching a null buffer to a layer surface unmaps it.\n\nUnmapping a layer_surface means that the surface cannot be shown by the\ncompositor until it is explicitly mapped again. The layer_surface\nreturns to the state it had right after layer_shell.get_layer_surface.\nThe client can re-map the surface by performing a commit without any\nbuffer attached, waiting for a configure event and handling it as usual."]
pub mod zwlr_layer_surface_v1 {
    use super::sys::common::{wl_argument, wl_array, wl_interface, wl_message};
    use super::sys::server::*;
    use super::{
        smallvec, types_null, AnonymousObject, Argument, ArgumentType, Interface, Main, Message,
        MessageDesc, MessageGroup, Object, ObjectMetadata, Resource, NULLPTR,
    };
    use std::os::raw::c_char;
    #[doc = "types of keyboard interaction possible for a layer shell surface\n\nTypes of keyboard interaction possible for layer shell surfaces. The\nrationale for this is twofold: (1) some applications are not interested\nin keyboard events and not allowing them to be focused can improve the\ndesktop experience; (2) some applications will want to take exclusive\nkeyboard focus."]
    #[repr(u32)]
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[non_exhaustive]
    pub enum KeyboardInteractivity {
        #[doc = "no keyboard focus is possible\n\nThis value indicates that this surface is not interested in keyboard\nevents and the compositor should never assign it the keyboard focus.\n\nThis is the default value, set for newly created layer shell surfaces.\n\nThis is useful for e.g. desktop widgets that display information or\nonly have interaction with non-keyboard input devices."]
        None = 0,
        #[doc = "request exclusive keyboard focus\n\nRequest exclusive keyboard focus if this surface is above the shell surface layer.\n\nFor the top and overlay layers, the seat will always give\nexclusive keyboard focus to the top-most layer which has keyboard\ninteractivity set to exclusive. If this layer contains multiple\nsurfaces with keyboard interactivity set to exclusive, the compositor\ndetermines the one receiving keyboard events in an implementation-\ndefined manner. In this case, no guarantee is made when this surface\nwill receive keyboard focus (if ever).\n\nFor the bottom and background layers, the compositor is allowed to use\nnormal focus semantics.\n\nThis setting is mainly intended for applications that need to ensure\nthey receive all keyboard events, such as a lock screen or a password\nprompt."]
        Exclusive = 1,
        #[doc = "request regular keyboard focus semantics\n\nThis requests the compositor to allow this surface to be focused and\nunfocused by the user in an implementation-defined manner. The user\nshould be able to unfocus this surface even regardless of the layer\nit is on.\n\nTypically, the compositor will want to use its normal mechanism to\nmanage keyboard focus between layer shell surfaces with this setting\nand regular toplevels on the desktop layer (e.g. click to focus).\nNevertheless, it is possible for a compositor to require a special\ninteraction to focus or unfocus layer shell surfaces (e.g. requiring\na click even if focus follows the mouse normally, or providing a\nkeybinding to switch focus between layers).\n\nThis setting is mainly intended for desktop shell components (e.g.\npanels) that allow keyboard interaction. Using this option can allow\nimplementing a desktop shell that can be fully usable without the\nmouse."]
        OnDemand = 2,
    }
    impl KeyboardInteractivity {
        pub fn from_raw(n: u32) -> Option<KeyboardInteractivity> {
            match n {
                0 => Some(KeyboardInteractivity::None),
                1 => Some(KeyboardInteractivity::Exclusive),
                2 => Some(KeyboardInteractivity::OnDemand),
                _ => Option::None,
            }
        }
        pub fn to_raw(&self) -> u32 {
            *self as u32
        }
    }
    #[repr(u32)]
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[non_exhaustive]
    pub enum Error {
        #[doc = "provided surface state is invalid"]
        InvalidSurfaceState = 0,
        #[doc = "size is invalid"]
        InvalidSize = 1,
        #[doc = "anchor bitfield is invalid"]
        InvalidAnchor = 2,
        #[doc = "keyboard interactivity is invalid"]
        InvalidKeyboardInteractivity = 3,
    }
    impl Error {
        pub fn from_raw(n: u32) -> Option<Error> {
            match n {
                0 => Some(Error::InvalidSurfaceState),
                1 => Some(Error::InvalidSize),
                2 => Some(Error::InvalidAnchor),
                3 => Some(Error::InvalidKeyboardInteractivity),
                _ => Option::None,
            }
        }
        pub fn to_raw(&self) -> u32 {
            *self as u32
        }
    }
    bitflags! { pub struct Anchor : u32 { # [doc = "the top edge of the anchor rectangle"] const Top = 1 ; # [doc = "the bottom edge of the anchor rectangle"] const Bottom = 2 ; # [doc = "the left edge of the anchor rectangle"] const Left = 4 ; # [doc = "the right edge of the anchor rectangle"] const Right = 8 ; } }
    impl Anchor {
        pub fn from_raw(n: u32) -> Option<Anchor> {
            Some(Anchor::from_bits_truncate(n))
        }
        pub fn to_raw(&self) -> u32 {
            self.bits()
        }
    }
    #[derive(Debug)]
    #[non_exhaustive]
    pub enum Request {
        #[doc = "sets the size of the surface\n\nSets the size of the surface in surface-local coordinates. The\ncompositor will display the surface centered with respect to its\nanchors.\n\nIf you pass 0 for either value, the compositor will assign it and\ninform you of the assignment in the configure event. You must set your\nanchor to opposite edges in the dimensions you omit; not doing so is a\nprotocol error. Both values are 0 by default.\n\nSize is double-buffered, see wl_surface.commit."]
        SetSize { width: u32, height: u32 },
        #[doc = "configures the anchor point of the surface\n\nRequests that the compositor anchor the surface to the specified edges\nand corners. If two orthogonal edges are specified (e.g. 'top' and\n'left'), then the anchor point will be the intersection of the edges\n(e.g. the top left corner of the output); otherwise the anchor point\nwill be centered on that edge, or in the center if none is specified.\n\nAnchor is double-buffered, see wl_surface.commit."]
        SetAnchor { anchor: Anchor },
        #[doc = "configures the exclusive geometry of this surface\n\nRequests that the compositor avoids occluding an area with other\nsurfaces. The compositor's use of this information is\nimplementation-dependent - do not assume that this region will not\nactually be occluded.\n\nA positive value is only meaningful if the surface is anchored to one\nedge or an edge and both perpendicular edges. If the surface is not\nanchored, anchored to only two perpendicular edges (a corner), anchored\nto only two parallel edges or anchored to all edges, a positive value\nwill be treated the same as zero.\n\nA positive zone is the distance from the edge in surface-local\ncoordinates to consider exclusive.\n\nSurfaces that do not wish to have an exclusive zone may instead specify\nhow they should interact with surfaces that do. If set to zero, the\nsurface indicates that it would like to be moved to avoid occluding\nsurfaces with a positive exclusive zone. If set to -1, the surface\nindicates that it would not like to be moved to accommodate for other\nsurfaces, and the compositor should extend it all the way to the edges\nit is anchored to.\n\nFor example, a panel might set its exclusive zone to 10, so that\nmaximized shell surfaces are not shown on top of it. A notification\nmight set its exclusive zone to 0, so that it is moved to avoid\noccluding the panel, but shell surfaces are shown underneath it. A\nwallpaper or lock screen might set their exclusive zone to -1, so that\nthey stretch below or over the panel.\n\nThe default value is 0.\n\nExclusive zone is double-buffered, see wl_surface.commit."]
        SetExclusiveZone { zone: i32 },
        #[doc = "sets a margin from the anchor point\n\nRequests that the surface be placed some distance away from the anchor\npoint on the output, in surface-local coordinates. Setting this value\nfor edges you are not anchored to has no effect.\n\nThe exclusive zone includes the margin.\n\nMargin is double-buffered, see wl_surface.commit."]
        SetMargin {
            top: i32,
            right: i32,
            bottom: i32,
            left: i32,
        },
        #[doc = "requests keyboard events\n\nSet how keyboard events are delivered to this surface. By default,\nlayer shell surfaces do not receive keyboard events; this request can\nbe used to change this.\n\nThis setting is inherited by child surfaces set by the get_popup\nrequest.\n\nLayer surfaces receive pointer, touch, and tablet events normally. If\nyou do not want to receive them, set the input region on your surface\nto an empty region.\n\nKeyboard interactivity is double-buffered, see wl_surface.commit."]
        SetKeyboardInteractivity {
            keyboard_interactivity: KeyboardInteractivity,
        },
        #[doc = "assign this layer_surface as an xdg_popup parent\n\nThis assigns an xdg_popup's parent to this layer_surface.  This popup\nshould have been created via xdg_surface::get_popup with the parent set\nto NULL, and this request must be invoked before committing the popup's\ninitial state.\n\nSee the documentation of xdg_popup for more details about what an\nxdg_popup is and how it is used."]
        GetPopup { popup: super::xdg_popup::XdgPopup },
        #[doc = "ack a configure event\n\nWhen a configure event is received, if a client commits the\nsurface in response to the configure event, then the client\nmust make an ack_configure request sometime before the commit\nrequest, passing along the serial of the configure event.\n\nIf the client receives multiple configure events before it\ncan respond to one, it only has to ack the last configure event.\n\nA client is not required to commit immediately after sending\nan ack_configure request - it may even ack_configure several times\nbefore its next surface commit.\n\nA client may send multiple ack_configure requests before committing, but\nonly the last request sent before a commit indicates which configure\nevent the client really is responding to."]
        AckConfigure { serial: u32 },
        #[doc = "destroy the layer_surface\n\nThis request destroys the layer surface.\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
        #[doc = "change the layer of the surface\n\nChange the layer that the surface is rendered on.\n\nLayer is double-buffered, see wl_surface.commit.\n\nOnly available since version 2 of the interface"]
        SetLayer {
            layer: super::zwlr_layer_shell_v1::Layer,
        },
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "set_size",
                since: 1,
                signature: &[super::ArgumentType::Uint, super::ArgumentType::Uint],
                destructor: false,
            },
            super::MessageDesc {
                name: "set_anchor",
                since: 1,
                signature: &[super::ArgumentType::Uint],
                destructor: false,
            },
            super::MessageDesc {
                name: "set_exclusive_zone",
                since: 1,
                signature: &[super::ArgumentType::Int],
                destructor: false,
            },
            super::MessageDesc {
                name: "set_margin",
                since: 1,
                signature: &[
                    super::ArgumentType::Int,
                    super::ArgumentType::Int,
                    super::ArgumentType::Int,
                    super::ArgumentType::Int,
                ],
                destructor: false,
            },
            super::MessageDesc {
                name: "set_keyboard_interactivity",
                since: 1,
                signature: &[super::ArgumentType::Uint],
                destructor: false,
            },
            super::MessageDesc {
                name: "get_popup",
                since: 1,
                signature: &[super::ArgumentType::Object],
                destructor: false,
            },
            super::MessageDesc {
                name: "ack_configure",
                since: 1,
                signature: &[super::ArgumentType::Uint],
                destructor: false,
            },
            super::MessageDesc {
                name: "destroy",
                since: 1,
                signature: &[],
                destructor: true,
            },
            super::MessageDesc {
                name: "set_layer",
                since: 2,
                signature: &[super::ArgumentType::Uint],
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
                Request::SetSize { .. } => 0,
                Request::SetAnchor { .. } => 1,
                Request::SetExclusiveZone { .. } => 2,
                Request::SetMargin { .. } => 3,
                Request::SetKeyboardInteractivity { .. } => 4,
                Request::GetPopup { .. } => 5,
                Request::AckConfigure { .. } => 6,
                Request::Destroy => 7,
                Request::SetLayer { .. } => 8,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::SetSize { .. } => 1,
                Request::SetAnchor { .. } => 1,
                Request::SetExclusiveZone { .. } => 1,
                Request::SetMargin { .. } => 1,
                Request::SetKeyboardInteractivity { .. } => 1,
                Request::GetPopup { .. } => 1,
                Request::AckConfigure { .. } => 1,
                Request::Destroy => 1,
                Request::SetLayer { .. } => 2,
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
                    Ok(Request::SetSize {
                        width: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        height: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                1 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::SetAnchor {
                        anchor: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                Anchor::from_raw(val).ok_or(())?
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                2 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::SetExclusiveZone {
                        zone: {
                            if let Some(Argument::Int(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                3 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::SetMargin {
                        top: {
                            if let Some(Argument::Int(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        right: {
                            if let Some(Argument::Int(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        bottom: {
                            if let Some(Argument::Int(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        left: {
                            if let Some(Argument::Int(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                4 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::SetKeyboardInteractivity {
                        keyboard_interactivity: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                KeyboardInteractivity::from_raw(val).ok_or(())?
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                5 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::GetPopup {
                        popup: {
                            if let Some(Argument::Object(val)) = args.next() {
                                map.get(val).ok_or(())?.into()
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                6 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::AckConfigure {
                        serial: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                7 => Ok(Request::Destroy),
                8 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::SetLayer {
                        layer: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                super::zwlr_layer_shell_v1::Layer::from_raw(val).ok_or(())?
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
                    Ok(Request::SetSize {
                        width: _args[0].u,
                        height: _args[1].u,
                    })
                }
                1 => {
                    let _args = ::std::slice::from_raw_parts(args, 1);
                    Ok(Request::SetAnchor {
                        anchor: Anchor::from_raw(_args[0].u).ok_or(())?,
                    })
                }
                2 => {
                    let _args = ::std::slice::from_raw_parts(args, 1);
                    Ok(Request::SetExclusiveZone { zone: _args[0].i })
                }
                3 => {
                    let _args = ::std::slice::from_raw_parts(args, 4);
                    Ok(Request::SetMargin {
                        top: _args[0].i,
                        right: _args[1].i,
                        bottom: _args[2].i,
                        left: _args[3].i,
                    })
                }
                4 => {
                    let _args = ::std::slice::from_raw_parts(args, 1);
                    Ok(Request::SetKeyboardInteractivity {
                        keyboard_interactivity: KeyboardInteractivity::from_raw(_args[0].u)
                            .ok_or(())?,
                    })
                }
                5 => {
                    let _args = ::std::slice::from_raw_parts(args, 1);
                    Ok(Request::GetPopup {
                        popup: Resource::<super::xdg_popup::XdgPopup>::from_c_ptr(
                            _args[0].o as *mut _,
                        )
                        .into(),
                    })
                }
                6 => {
                    let _args = ::std::slice::from_raw_parts(args, 1);
                    Ok(Request::AckConfigure { serial: _args[0].u })
                }
                7 => Ok(Request::Destroy),
                8 => {
                    let _args = ::std::slice::from_raw_parts(args, 1);
                    Ok(Request::SetLayer {
                        layer: super::zwlr_layer_shell_v1::Layer::from_raw(_args[0].u).ok_or(())?,
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
        #[doc = "suggest a surface change\n\nThe configure event asks the client to resize its surface.\n\nClients should arrange their surface for the new states, and then send\nan ack_configure request with the serial sent in this configure event at\nsome point before committing the new surface.\n\nThe client is free to dismiss all but the last configure event it\nreceived.\n\nThe width and height arguments specify the size of the window in\nsurface-local coordinates.\n\nThe size is a hint, in the sense that the client is free to ignore it if\nit doesn't resize, pick a smaller size (to satisfy aspect ratio or\nresize in steps of NxM pixels). If the client picks a smaller size and\nis anchored to two opposite anchors (e.g. 'top' and 'bottom'), the\nsurface will be centered on this axis.\n\nIf the width or height arguments are zero, it means the client should\ndecide its own window dimension."]
        Configure {
            serial: u32,
            width: u32,
            height: u32,
        },
        #[doc = "surface should be closed\n\nThe closed event is sent by the compositor when the surface will no\nlonger be shown. The output may have been destroyed or the user may\nhave asked for it to be removed. Further changes to the surface will be\nignored. The client should destroy the resource after receiving this\nevent, and create a new surface if they so choose."]
        Closed,
    }
    impl super::MessageGroup for Event {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "configure",
                since: 1,
                signature: &[
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                ],
                destructor: false,
            },
            super::MessageDesc {
                name: "closed",
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
                Event::Configure { .. } => 0,
                Event::Closed => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Event::Configure { .. } => 1,
                Event::Closed => 1,
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
                Event::Configure {
                    serial,
                    width,
                    height,
                } => Message {
                    sender_id: sender_id,
                    opcode: 0,
                    args: smallvec![
                        Argument::Uint(serial),
                        Argument::Uint(width),
                        Argument::Uint(height),
                    ],
                },
                Event::Closed => Message {
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
                Event::Configure {
                    serial,
                    width,
                    height,
                } => {
                    let mut _args_array: [wl_argument; 3] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = serial;
                    _args_array[1].u = width;
                    _args_array[2].u = height;
                    f(0, &mut _args_array)
                }
                Event::Closed => {
                    let mut _args_array: [wl_argument; 0] = unsafe { ::std::mem::zeroed() };
                    f(1, &mut _args_array)
                }
            }
        }
    }
    #[derive(Clone, Eq, PartialEq)]
    pub struct ZwlrLayerSurfaceV1(Resource<ZwlrLayerSurfaceV1>);
    impl AsRef<Resource<ZwlrLayerSurfaceV1>> for ZwlrLayerSurfaceV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwlrLayerSurfaceV1>> for ZwlrLayerSurfaceV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwlrLayerSurfaceV1(value)
        }
    }
    impl From<ZwlrLayerSurfaceV1> for Resource<ZwlrLayerSurfaceV1> {
        #[inline]
        fn from(value: ZwlrLayerSurfaceV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwlrLayerSurfaceV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwlrLayerSurfaceV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwlr_layer_surface_v1";
        const VERSION: u32 = 4;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwlr_layer_surface_v1_interface }
        }
    }
    impl ZwlrLayerSurfaceV1 {
        #[doc = "suggest a surface change\n\nThe configure event asks the client to resize its surface.\n\nClients should arrange their surface for the new states, and then send\nan ack_configure request with the serial sent in this configure event at\nsome point before committing the new surface.\n\nThe client is free to dismiss all but the last configure event it\nreceived.\n\nThe width and height arguments specify the size of the window in\nsurface-local coordinates.\n\nThe size is a hint, in the sense that the client is free to ignore it if\nit doesn't resize, pick a smaller size (to satisfy aspect ratio or\nresize in steps of NxM pixels). If the client picks a smaller size and\nis anchored to two opposite anchors (e.g. 'top' and 'bottom'), the\nsurface will be centered on this axis.\n\nIf the width or height arguments are zero, it means the client should\ndecide its own window dimension."]
        pub fn configure(&self, serial: u32, width: u32, height: u32) -> () {
            let msg = Event::Configure {
                serial: serial,
                width: width,
                height: height,
            };
            self.0.send(msg);
        }
        #[doc = "surface should be closed\n\nThe closed event is sent by the compositor when the surface will no\nlonger be shown. The output may have been destroyed or the user may\nhave asked for it to be removed. Further changes to the surface will be\nignored. The client should destroy the resource after receiving this\nevent, and create a new surface if they so choose."]
        pub fn closed(&self) -> () {
            let msg = Event::Closed;
            self.0.send(msg);
        }
    }
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_SET_SIZE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_SET_ANCHOR_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_SET_EXCLUSIVE_ZONE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_SET_MARGIN_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_SET_KEYBOARD_INTERACTIVITY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_GET_POPUP_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_ACK_CONFIGURE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_SET_LAYER_SINCE: u32 = 2u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_CONFIGURE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_CLOSED_SINCE: u32 = 1u32;
    static mut zwlr_layer_surface_v1_requests_get_popup_types: [*const wl_interface; 1] =
        [unsafe { &super::xdg_popup::xdg_popup_interface as *const wl_interface }];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwlr_layer_surface_v1_requests: [wl_message; 9] = [
        wl_message {
            name: b"set_size\0" as *const u8 as *const c_char,
            signature: b"uu\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"set_anchor\0" as *const u8 as *const c_char,
            signature: b"u\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"set_exclusive_zone\0" as *const u8 as *const c_char,
            signature: b"i\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"set_margin\0" as *const u8 as *const c_char,
            signature: b"iiii\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"set_keyboard_interactivity\0" as *const u8 as *const c_char,
            signature: b"u\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"get_popup\0" as *const u8 as *const c_char,
            signature: b"o\0" as *const u8 as *const c_char,
            types: unsafe { &zwlr_layer_surface_v1_requests_get_popup_types as *const _ },
        },
        wl_message {
            name: b"ack_configure\0" as *const u8 as *const c_char,
            signature: b"u\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"set_layer\0" as *const u8 as *const c_char,
            signature: b"2u\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwlr_layer_surface_v1_events: [wl_message; 2] = [
        wl_message {
            name: b"configure\0" as *const u8 as *const c_char,
            signature: b"uuu\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"closed\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwlr_layer_surface_v1_interface: wl_interface = wl_interface {
        name: b"zwlr_layer_surface_v1\0" as *const u8 as *const c_char,
        version: 4,
        request_count: 9,
        requests: unsafe { &zwlr_layer_surface_v1_requests as *const _ },
        event_count: 2,
        events: unsafe { &zwlr_layer_surface_v1_events as *const _ },
    };
}
