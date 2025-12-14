use std::os::raw::{c_char, c_void};
const NULLPTR: *const c_void = 0 as *const c_void;
static mut types_null: [*const sys::common::wl_interface; 5] = [
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
];
#[doc = "input method context\n\nCorresponds to a text input on the input method side. An input method context\nis created on text input activation on the input method side. It allows\nreceiving information about the text input from the application via events.\nInput method contexts do not keep state after deactivation and should be\ndestroyed after deactivation is handled.\n\nText is generally UTF-8 encoded, indices and lengths are in bytes.\n\nSerials are used to synchronize the state between the text input and\nan input method. New serials are sent by the text input in the\ncommit_state request and are used by the input method to indicate\nthe known text input state in events like preedit_string, commit_string,\nand keysym. The text input can then ignore events from the input method\nwhich are based on an outdated state (for example after a reset).\n\nWarning! The protocol described in this file is experimental and\nbackward incompatible changes may be made. Backward compatible changes\nmay be added together with the corresponding interface version bump.\nBackward incompatible changes are done by bumping the version number in\nthe protocol and interface names and resetting the interface version.\nOnce the protocol is to be declared stable, the 'z' prefix and the\nversion number in the protocol and interface names are removed and the\ninterface version number is reset."]
pub mod zwp_input_method_context_v1 {
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
        #[doc = "This is a destructor, once received this object cannot be used any longer."]
        Destroy,
        #[doc = "commit string\n\nSend the commit string text for insertion to the application.\n\nThe text to commit could be either just a single character after a key\npress or the result of some composing (pre-edit). It could be also an\nempty text when some text should be removed (see\ndelete_surrounding_text) or when the input cursor should be moved (see\ncursor_position).\n\nAny previously set composing text will be removed."]
        CommitString { serial: u32, text: String },
        #[doc = "pre-edit string\n\nSend the pre-edit string text to the application text input.\n\nThe commit text can be used to replace the pre-edit text on reset (for\nexample on unfocus).\n\nPreviously sent preedit_style and preedit_cursor requests are also\nprocessed by the text_input."]
        PreeditString {
            serial: u32,
            text: String,
            commit: String,
        },
        #[doc = "pre-edit styling\n\nSet the styling information on composing text. The style is applied for\nlength in bytes from index relative to the beginning of\nthe composing text (as byte offset). Multiple styles can\nbe applied to a composing text.\n\nThis request should be sent before sending a preedit_string request."]
        PreeditStyling { index: u32, length: u32, style: u32 },
        #[doc = "pre-edit cursor\n\nSet the cursor position inside the composing text (as byte offset)\nrelative to the start of the composing text.\n\nWhen index is negative no cursor should be displayed.\n\nThis request should be sent before sending a preedit_string request."]
        PreeditCursor { index: i32 },
        #[doc = "delete text\n\nRemove the surrounding text.\n\nThis request will be handled on the text_input side directly following\na commit_string request."]
        DeleteSurroundingText { index: i32, length: u32 },
        #[doc = "set cursor to a new position\n\nSet the cursor and anchor to a new position. Index is the new cursor\nposition in bytes (when >= 0 this is relative to the end of the inserted text,\notherwise it is relative to the beginning of the inserted text). Anchor is\nthe new anchor position in bytes (when >= 0 this is relative to the end of the\ninserted text, otherwise it is relative to the beginning of the inserted\ntext). When there should be no selected text, anchor should be the same\nas index.\n\nThis request will be handled on the text_input side directly following\na commit_string request."]
        CursorPosition { index: i32, anchor: i32 },
        #[doc = ""]
        ModifiersMap { map: Vec<u8> },
        #[doc = "keysym\n\nNotify when a key event was sent. Key events should not be used for\nnormal text input operations, which should be done with commit_string,\ndelete_surrounding_text, etc. The key event follows the wl_keyboard key\nevent convention. Sym is an XKB keysym, state is a wl_keyboard key_state."]
        Keysym {
            serial: u32,
            time: u32,
            sym: u32,
            state: u32,
            modifiers: u32,
        },
        #[doc = "grab hardware keyboard\n\nAllow an input method to receive hardware keyboard input and process\nkey events to generate text events (with pre-edit) over the wire. This\nallows input methods which compose multiple key events for inputting\ntext like it is done for CJK languages."]
        GrabKeyboard {
            keyboard: Main<super::wl_keyboard::WlKeyboard>,
        },
        #[doc = "forward key event\n\nForward a wl_keyboard::key event to the client that was not processed\nby the input method itself. Should be used when filtering key events\nwith grab_keyboard.  The arguments should be the ones from the\nwl_keyboard::key event.\n\nFor generating custom key events use the keysym request instead."]
        Key {
            serial: u32,
            time: u32,
            key: u32,
            state: u32,
        },
        #[doc = "forward modifiers event\n\nForward a wl_keyboard::modifiers event to the client that was not\nprocessed by the input method itself.  Should be used when filtering\nkey events with grab_keyboard. The arguments should be the ones\nfrom the wl_keyboard::modifiers event."]
        Modifiers {
            serial: u32,
            mods_depressed: u32,
            mods_latched: u32,
            mods_locked: u32,
            group: u32,
        },
        #[doc = ""]
        Language { serial: u32, language: String },
        #[doc = ""]
        TextDirection { serial: u32, direction: u32 },
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
                name: "commit_string",
                since: 1,
                signature: &[super::ArgumentType::Uint, super::ArgumentType::Str],
                destructor: false,
            },
            super::MessageDesc {
                name: "preedit_string",
                since: 1,
                signature: &[
                    super::ArgumentType::Uint,
                    super::ArgumentType::Str,
                    super::ArgumentType::Str,
                ],
                destructor: false,
            },
            super::MessageDesc {
                name: "preedit_styling",
                since: 1,
                signature: &[
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                ],
                destructor: false,
            },
            super::MessageDesc {
                name: "preedit_cursor",
                since: 1,
                signature: &[super::ArgumentType::Int],
                destructor: false,
            },
            super::MessageDesc {
                name: "delete_surrounding_text",
                since: 1,
                signature: &[super::ArgumentType::Int, super::ArgumentType::Uint],
                destructor: false,
            },
            super::MessageDesc {
                name: "cursor_position",
                since: 1,
                signature: &[super::ArgumentType::Int, super::ArgumentType::Int],
                destructor: false,
            },
            super::MessageDesc {
                name: "modifiers_map",
                since: 1,
                signature: &[super::ArgumentType::Array],
                destructor: false,
            },
            super::MessageDesc {
                name: "keysym",
                since: 1,
                signature: &[
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                ],
                destructor: false,
            },
            super::MessageDesc {
                name: "grab_keyboard",
                since: 1,
                signature: &[super::ArgumentType::NewId],
                destructor: false,
            },
            super::MessageDesc {
                name: "key",
                since: 1,
                signature: &[
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                ],
                destructor: false,
            },
            super::MessageDesc {
                name: "modifiers",
                since: 1,
                signature: &[
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                ],
                destructor: false,
            },
            super::MessageDesc {
                name: "language",
                since: 1,
                signature: &[super::ArgumentType::Uint, super::ArgumentType::Str],
                destructor: false,
            },
            super::MessageDesc {
                name: "text_direction",
                since: 1,
                signature: &[super::ArgumentType::Uint, super::ArgumentType::Uint],
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
                Request::CommitString { .. } => 1,
                Request::PreeditString { .. } => 2,
                Request::PreeditStyling { .. } => 3,
                Request::PreeditCursor { .. } => 4,
                Request::DeleteSurroundingText { .. } => 5,
                Request::CursorPosition { .. } => 6,
                Request::ModifiersMap { .. } => 7,
                Request::Keysym { .. } => 8,
                Request::GrabKeyboard { .. } => 9,
                Request::Key { .. } => 10,
                Request::Modifiers { .. } => 11,
                Request::Language { .. } => 12,
                Request::TextDirection { .. } => 13,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::Destroy => 1,
                Request::CommitString { .. } => 1,
                Request::PreeditString { .. } => 1,
                Request::PreeditStyling { .. } => 1,
                Request::PreeditCursor { .. } => 1,
                Request::DeleteSurroundingText { .. } => 1,
                Request::CursorPosition { .. } => 1,
                Request::ModifiersMap { .. } => 1,
                Request::Keysym { .. } => 1,
                Request::GrabKeyboard { .. } => 1,
                Request::Key { .. } => 1,
                Request::Modifiers { .. } => 1,
                Request::Language { .. } => 1,
                Request::TextDirection { .. } => 1,
            }
        }
        fn child<Meta: ObjectMetadata>(
            opcode: u16,
            version: u32,
            meta: &Meta,
        ) -> Option<Object<Meta>> {
            match opcode {
                9 => Some(Object::from_interface::<super::wl_keyboard::WlKeyboard>(
                    version,
                    meta.child(),
                )),
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            match msg.opcode {
                0 => Ok(Request::Destroy),
                1 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::CommitString {
                        serial: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        text: {
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
                2 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::PreeditString {
                        serial: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        text: {
                            if let Some(Argument::Str(val)) = args.next() {
                                let s = String::from_utf8(val.into_bytes()).unwrap_or_else(|e| {
                                    String::from_utf8_lossy(&e.into_bytes()).into()
                                });
                                s
                            } else {
                                return Err(());
                            }
                        },
                        commit: {
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
                3 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::PreeditStyling {
                        index: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        length: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        style: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                4 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::PreeditCursor {
                        index: {
                            if let Some(Argument::Int(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                5 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::DeleteSurroundingText {
                        index: {
                            if let Some(Argument::Int(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        length: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                6 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::CursorPosition {
                        index: {
                            if let Some(Argument::Int(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        anchor: {
                            if let Some(Argument::Int(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                7 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::ModifiersMap {
                        map: {
                            if let Some(Argument::Array(val)) = args.next() {
                                *val
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                8 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::Keysym {
                        serial: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        time: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        sym: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        state: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        modifiers: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                9 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::GrabKeyboard {
                        keyboard: {
                            if let Some(Argument::NewId(val)) = args.next() {
                                map.get_new(val).ok_or(())?
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                10 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::Key {
                        serial: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        time: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        key: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        state: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                11 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::Modifiers {
                        serial: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        mods_depressed: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        mods_latched: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        mods_locked: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        group: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                12 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::Language {
                        serial: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        language: {
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
                13 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::TextDirection {
                        serial: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        direction: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
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
                    Ok(Request::CommitString {
                        serial: _args[0].u,
                        text: ::std::ffi::CStr::from_ptr(_args[1].s)
                            .to_string_lossy()
                            .into_owned(),
                    })
                }
                2 => {
                    let _args = ::std::slice::from_raw_parts(args, 3);
                    Ok(Request::PreeditString {
                        serial: _args[0].u,
                        text: ::std::ffi::CStr::from_ptr(_args[1].s)
                            .to_string_lossy()
                            .into_owned(),
                        commit: ::std::ffi::CStr::from_ptr(_args[2].s)
                            .to_string_lossy()
                            .into_owned(),
                    })
                }
                3 => {
                    let _args = ::std::slice::from_raw_parts(args, 3);
                    Ok(Request::PreeditStyling {
                        index: _args[0].u,
                        length: _args[1].u,
                        style: _args[2].u,
                    })
                }
                4 => {
                    let _args = ::std::slice::from_raw_parts(args, 1);
                    Ok(Request::PreeditCursor { index: _args[0].i })
                }
                5 => {
                    let _args = ::std::slice::from_raw_parts(args, 2);
                    Ok(Request::DeleteSurroundingText {
                        index: _args[0].i,
                        length: _args[1].u,
                    })
                }
                6 => {
                    let _args = ::std::slice::from_raw_parts(args, 2);
                    Ok(Request::CursorPosition {
                        index: _args[0].i,
                        anchor: _args[1].i,
                    })
                }
                7 => {
                    let _args = ::std::slice::from_raw_parts(args, 1);
                    Ok(Request::ModifiersMap {
                        map: {
                            let array = &*_args[0].a;
                            ::std::slice::from_raw_parts(array.data as *const u8, array.size)
                                .to_owned()
                        },
                    })
                }
                8 => {
                    let _args = ::std::slice::from_raw_parts(args, 5);
                    Ok(Request::Keysym {
                        serial: _args[0].u,
                        time: _args[1].u,
                        sym: _args[2].u,
                        state: _args[3].u,
                        modifiers: _args[4].u,
                    })
                }
                9 => {
                    let _args = ::std::slice::from_raw_parts(args, 1);
                    Ok(Request::GrabKeyboard {
                        keyboard: {
                            let me = Resource::<ZwpInputMethodContextV1>::from_c_ptr(obj as *mut _);
                            me.make_child_for::<super::wl_keyboard::WlKeyboard>(_args[0].n)
                                .unwrap()
                        },
                    })
                }
                10 => {
                    let _args = ::std::slice::from_raw_parts(args, 4);
                    Ok(Request::Key {
                        serial: _args[0].u,
                        time: _args[1].u,
                        key: _args[2].u,
                        state: _args[3].u,
                    })
                }
                11 => {
                    let _args = ::std::slice::from_raw_parts(args, 5);
                    Ok(Request::Modifiers {
                        serial: _args[0].u,
                        mods_depressed: _args[1].u,
                        mods_latched: _args[2].u,
                        mods_locked: _args[3].u,
                        group: _args[4].u,
                    })
                }
                12 => {
                    let _args = ::std::slice::from_raw_parts(args, 2);
                    Ok(Request::Language {
                        serial: _args[0].u,
                        language: ::std::ffi::CStr::from_ptr(_args[1].s)
                            .to_string_lossy()
                            .into_owned(),
                    })
                }
                13 => {
                    let _args = ::std::slice::from_raw_parts(args, 2);
                    Ok(Request::TextDirection {
                        serial: _args[0].u,
                        direction: _args[1].u,
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
        #[doc = "surrounding text event\n\nThe plain surrounding text around the input position. Cursor is the\nposition in bytes within the surrounding text relative to the beginning\nof the text. Anchor is the position in bytes of the selection anchor\nwithin the surrounding text relative to the beginning of the text. If\nthere is no selected text then anchor is the same as cursor."]
        SurroundingText {
            text: String,
            cursor: u32,
            anchor: u32,
        },
        #[doc = ""]
        Reset,
        #[doc = ""]
        ContentType { hint: u32, purpose: u32 },
        #[doc = ""]
        InvokeAction { button: u32, index: u32 },
        #[doc = ""]
        CommitState { serial: u32 },
        #[doc = ""]
        PreferredLanguage { language: String },
    }
    impl super::MessageGroup for Event {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "surrounding_text",
                since: 1,
                signature: &[
                    super::ArgumentType::Str,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                ],
                destructor: false,
            },
            super::MessageDesc {
                name: "reset",
                since: 1,
                signature: &[],
                destructor: false,
            },
            super::MessageDesc {
                name: "content_type",
                since: 1,
                signature: &[super::ArgumentType::Uint, super::ArgumentType::Uint],
                destructor: false,
            },
            super::MessageDesc {
                name: "invoke_action",
                since: 1,
                signature: &[super::ArgumentType::Uint, super::ArgumentType::Uint],
                destructor: false,
            },
            super::MessageDesc {
                name: "commit_state",
                since: 1,
                signature: &[super::ArgumentType::Uint],
                destructor: false,
            },
            super::MessageDesc {
                name: "preferred_language",
                since: 1,
                signature: &[super::ArgumentType::Str],
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
                Event::SurroundingText { .. } => 0,
                Event::Reset => 1,
                Event::ContentType { .. } => 2,
                Event::InvokeAction { .. } => 3,
                Event::CommitState { .. } => 4,
                Event::PreferredLanguage { .. } => 5,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Event::SurroundingText { .. } => 1,
                Event::Reset => 1,
                Event::ContentType { .. } => 1,
                Event::InvokeAction { .. } => 1,
                Event::CommitState { .. } => 1,
                Event::PreferredLanguage { .. } => 1,
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
                Event::SurroundingText {
                    text,
                    cursor,
                    anchor,
                } => Message {
                    sender_id: sender_id,
                    opcode: 0,
                    args: smallvec![
                        Argument::Str(Box::new(unsafe {
                            ::std::ffi::CString::from_vec_unchecked(text.into())
                        })),
                        Argument::Uint(cursor),
                        Argument::Uint(anchor),
                    ],
                },
                Event::Reset => Message {
                    sender_id: sender_id,
                    opcode: 1,
                    args: smallvec![],
                },
                Event::ContentType { hint, purpose } => Message {
                    sender_id: sender_id,
                    opcode: 2,
                    args: smallvec![Argument::Uint(hint), Argument::Uint(purpose),],
                },
                Event::InvokeAction { button, index } => Message {
                    sender_id: sender_id,
                    opcode: 3,
                    args: smallvec![Argument::Uint(button), Argument::Uint(index),],
                },
                Event::CommitState { serial } => Message {
                    sender_id: sender_id,
                    opcode: 4,
                    args: smallvec![Argument::Uint(serial),],
                },
                Event::PreferredLanguage { language } => Message {
                    sender_id: sender_id,
                    opcode: 5,
                    args: smallvec![Argument::Str(Box::new(unsafe {
                        ::std::ffi::CString::from_vec_unchecked(language.into())
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
                Event::SurroundingText {
                    text,
                    cursor,
                    anchor,
                } => {
                    let mut _args_array: [wl_argument; 3] = unsafe { ::std::mem::zeroed() };
                    let _arg_0 = ::std::ffi::CString::new(text).unwrap();
                    _args_array[0].s = _arg_0.as_ptr();
                    _args_array[1].u = cursor;
                    _args_array[2].u = anchor;
                    f(0, &mut _args_array)
                }
                Event::Reset => {
                    let mut _args_array: [wl_argument; 0] = unsafe { ::std::mem::zeroed() };
                    f(1, &mut _args_array)
                }
                Event::ContentType { hint, purpose } => {
                    let mut _args_array: [wl_argument; 2] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = hint;
                    _args_array[1].u = purpose;
                    f(2, &mut _args_array)
                }
                Event::InvokeAction { button, index } => {
                    let mut _args_array: [wl_argument; 2] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = button;
                    _args_array[1].u = index;
                    f(3, &mut _args_array)
                }
                Event::CommitState { serial } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = serial;
                    f(4, &mut _args_array)
                }
                Event::PreferredLanguage { language } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    let _arg_0 = ::std::ffi::CString::new(language).unwrap();
                    _args_array[0].s = _arg_0.as_ptr();
                    f(5, &mut _args_array)
                }
            }
        }
    }
    #[derive(Clone, Eq, PartialEq)]
    pub struct ZwpInputMethodContextV1(Resource<ZwpInputMethodContextV1>);
    impl AsRef<Resource<ZwpInputMethodContextV1>> for ZwpInputMethodContextV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwpInputMethodContextV1>> for ZwpInputMethodContextV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwpInputMethodContextV1(value)
        }
    }
    impl From<ZwpInputMethodContextV1> for Resource<ZwpInputMethodContextV1> {
        #[inline]
        fn from(value: ZwpInputMethodContextV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwpInputMethodContextV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwpInputMethodContextV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwp_input_method_context_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwp_input_method_context_v1_interface }
        }
    }
    impl ZwpInputMethodContextV1 {
        #[doc = "surrounding text event\n\nThe plain surrounding text around the input position. Cursor is the\nposition in bytes within the surrounding text relative to the beginning\nof the text. Anchor is the position in bytes of the selection anchor\nwithin the surrounding text relative to the beginning of the text. If\nthere is no selected text then anchor is the same as cursor."]
        pub fn surrounding_text(&self, text: String, cursor: u32, anchor: u32) -> () {
            let msg = Event::SurroundingText {
                text: text,
                cursor: cursor,
                anchor: anchor,
            };
            self.0.send(msg);
        }
        #[doc = ""]
        pub fn reset(&self) -> () {
            let msg = Event::Reset;
            self.0.send(msg);
        }
        #[doc = ""]
        pub fn content_type(&self, hint: u32, purpose: u32) -> () {
            let msg = Event::ContentType {
                hint: hint,
                purpose: purpose,
            };
            self.0.send(msg);
        }
        #[doc = ""]
        pub fn invoke_action(&self, button: u32, index: u32) -> () {
            let msg = Event::InvokeAction {
                button: button,
                index: index,
            };
            self.0.send(msg);
        }
        #[doc = ""]
        pub fn commit_state(&self, serial: u32) -> () {
            let msg = Event::CommitState { serial: serial };
            self.0.send(msg);
        }
        #[doc = ""]
        pub fn preferred_language(&self, language: String) -> () {
            let msg = Event::PreferredLanguage { language: language };
            self.0.send(msg);
        }
    }
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_COMMIT_STRING_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_PREEDIT_STRING_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_PREEDIT_STYLING_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_PREEDIT_CURSOR_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DELETE_SURROUNDING_TEXT_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_CURSOR_POSITION_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_MODIFIERS_MAP_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_KEYSYM_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_GRAB_KEYBOARD_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_KEY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_MODIFIERS_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_LANGUAGE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_TEXT_DIRECTION_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_SURROUNDING_TEXT_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_RESET_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_CONTENT_TYPE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_INVOKE_ACTION_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_COMMIT_STATE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_PREFERRED_LANGUAGE_SINCE: u32 = 1u32;
    static mut zwp_input_method_context_v1_requests_grab_keyboard_types: [*const wl_interface; 1] =
        [unsafe { &super::wl_keyboard::wl_keyboard_interface as *const wl_interface }];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_input_method_context_v1_requests: [wl_message; 14] = [
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"commit_string\0" as *const u8 as *const c_char,
            signature: b"us\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"preedit_string\0" as *const u8 as *const c_char,
            signature: b"uss\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"preedit_styling\0" as *const u8 as *const c_char,
            signature: b"uuu\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"preedit_cursor\0" as *const u8 as *const c_char,
            signature: b"i\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"delete_surrounding_text\0" as *const u8 as *const c_char,
            signature: b"iu\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"cursor_position\0" as *const u8 as *const c_char,
            signature: b"ii\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"modifiers_map\0" as *const u8 as *const c_char,
            signature: b"a\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"keysym\0" as *const u8 as *const c_char,
            signature: b"uuuuu\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"grab_keyboard\0" as *const u8 as *const c_char,
            signature: b"n\0" as *const u8 as *const c_char,
            types: unsafe { &zwp_input_method_context_v1_requests_grab_keyboard_types as *const _ },
        },
        wl_message {
            name: b"key\0" as *const u8 as *const c_char,
            signature: b"uuuu\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"modifiers\0" as *const u8 as *const c_char,
            signature: b"uuuuu\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"language\0" as *const u8 as *const c_char,
            signature: b"us\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"text_direction\0" as *const u8 as *const c_char,
            signature: b"uu\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_input_method_context_v1_events: [wl_message; 6] = [
        wl_message {
            name: b"surrounding_text\0" as *const u8 as *const c_char,
            signature: b"suu\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"reset\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"content_type\0" as *const u8 as *const c_char,
            signature: b"uu\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"invoke_action\0" as *const u8 as *const c_char,
            signature: b"uu\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"commit_state\0" as *const u8 as *const c_char,
            signature: b"u\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"preferred_language\0" as *const u8 as *const c_char,
            signature: b"s\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwp_input_method_context_v1_interface: wl_interface = wl_interface {
        name: b"zwp_input_method_context_v1\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 14,
        requests: unsafe { &zwp_input_method_context_v1_requests as *const _ },
        event_count: 6,
        events: unsafe { &zwp_input_method_context_v1_events as *const _ },
    };
}
#[doc = "input method\n\nAn input method object is responsible for composing text in response to\ninput from hardware or virtual keyboards. There is one input method\nobject per seat. On activate there is a new input method context object\ncreated which allows the input method to communicate with the text input."]
pub mod zwp_input_method_v1 {
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
        #[doc = "activate event\n\nA text input was activated. Creates an input method context object\nwhich allows communication with the text input."]
        Activate {
            id: Resource<super::zwp_input_method_context_v1::ZwpInputMethodContextV1>,
        },
        #[doc = "deactivate event\n\nThe text input corresponding to the context argument was deactivated.\nThe input method context should be destroyed after deactivation is\nhandled."]
        Deactivate {
            context: super::zwp_input_method_context_v1::ZwpInputMethodContextV1,
        },
    }
    impl super::MessageGroup for Event {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "activate",
                since: 1,
                signature: &[super::ArgumentType::NewId],
                destructor: false,
            },
            super::MessageDesc {
                name: "deactivate",
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
                Event::Activate { .. } => 0,
                Event::Deactivate { .. } => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Event::Activate { .. } => 1,
                Event::Deactivate { .. } => 1,
            }
        }
        fn child<Meta: ObjectMetadata>(
            opcode: u16,
            version: u32,
            meta: &Meta,
        ) -> Option<Object<Meta>> {
            match opcode {
                0 => Some(Object::from_interface::<
                    super::zwp_input_method_context_v1::ZwpInputMethodContextV1,
                >(version, meta.child())),
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            panic!("Event::from_raw can not be used Server-side.")
        }
        fn into_raw(self, sender_id: u32) -> Message {
            match self {
                Event::Activate { id } => Message {
                    sender_id: sender_id,
                    opcode: 0,
                    args: smallvec![Argument::NewId(id.id()),],
                },
                Event::Deactivate { context } => Message {
                    sender_id: sender_id,
                    opcode: 1,
                    args: smallvec![Argument::Object(context.as_ref().id()),],
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
                Event::Activate { id } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].o = id.c_ptr() as *mut _;
                    f(0, &mut _args_array)
                }
                Event::Deactivate { context } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].o = context.as_ref().c_ptr() as *mut _;
                    f(1, &mut _args_array)
                }
            }
        }
    }
    #[derive(Clone, Eq, PartialEq)]
    pub struct ZwpInputMethodV1(Resource<ZwpInputMethodV1>);
    impl AsRef<Resource<ZwpInputMethodV1>> for ZwpInputMethodV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwpInputMethodV1>> for ZwpInputMethodV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwpInputMethodV1(value)
        }
    }
    impl From<ZwpInputMethodV1> for Resource<ZwpInputMethodV1> {
        #[inline]
        fn from(value: ZwpInputMethodV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwpInputMethodV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwpInputMethodV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwp_input_method_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwp_input_method_v1_interface }
        }
    }
    impl ZwpInputMethodV1 {
        #[doc = "activate event\n\nA text input was activated. Creates an input method context object\nwhich allows communication with the text input."]
        pub fn activate(
            &self,
            id: &super::zwp_input_method_context_v1::ZwpInputMethodContextV1,
        ) -> () {
            let msg = Event::Activate {
                id: id.as_ref().clone(),
            };
            self.0.send(msg);
        }
        #[doc = "deactivate event\n\nThe text input corresponding to the context argument was deactivated.\nThe input method context should be destroyed after deactivation is\nhandled."]
        pub fn deactivate(
            &self,
            context: &super::zwp_input_method_context_v1::ZwpInputMethodContextV1,
        ) -> () {
            let msg = Event::Deactivate {
                context: context.clone(),
            };
            self.0.send(msg);
        }
    }
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_ACTIVATE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_DEACTIVATE_SINCE: u32 = 1u32;
    static mut zwp_input_method_v1_events_activate_types: [*const wl_interface; 1] = [unsafe {
        &super::zwp_input_method_context_v1::zwp_input_method_context_v1_interface
            as *const wl_interface
    }];
    static mut zwp_input_method_v1_events_deactivate_types: [*const wl_interface; 1] = [unsafe {
        &super::zwp_input_method_context_v1::zwp_input_method_context_v1_interface
            as *const wl_interface
    }];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_input_method_v1_events: [wl_message; 2] = [
        wl_message {
            name: b"activate\0" as *const u8 as *const c_char,
            signature: b"n\0" as *const u8 as *const c_char,
            types: unsafe { &zwp_input_method_v1_events_activate_types as *const _ },
        },
        wl_message {
            name: b"deactivate\0" as *const u8 as *const c_char,
            signature: b"o\0" as *const u8 as *const c_char,
            types: unsafe { &zwp_input_method_v1_events_deactivate_types as *const _ },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwp_input_method_v1_interface: wl_interface = wl_interface {
        name: b"zwp_input_method_v1\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 0,
        requests: NULLPTR as *const wl_message,
        event_count: 2,
        events: unsafe { &zwp_input_method_v1_events as *const _ },
    };
}
#[doc = "interface for implementing keyboards\n\nOnly one client can bind this interface at a time."]
pub mod zwp_input_panel_v1 {
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
        #[doc = ""]
        GetInputPanelSurface {
            id: Main<super::zwp_input_panel_surface_v1::ZwpInputPanelSurfaceV1>,
            surface: super::wl_surface::WlSurface,
        },
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[super::MessageDesc {
            name: "get_input_panel_surface",
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
                Request::GetInputPanelSurface { .. } => 0,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::GetInputPanelSurface { .. } => 1,
            }
        }
        fn child<Meta: ObjectMetadata>(
            opcode: u16,
            version: u32,
            meta: &Meta,
        ) -> Option<Object<Meta>> {
            match opcode {
                0 => Some(Object::from_interface::<
                    super::zwp_input_panel_surface_v1::ZwpInputPanelSurfaceV1,
                >(version, meta.child())),
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            match msg.opcode {
                0 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::GetInputPanelSurface {
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
                    Ok(Request::GetInputPanelSurface {
                        id: {
                            let me = Resource::<ZwpInputPanelV1>::from_c_ptr(obj as *mut _);
                            me . make_child_for :: < super :: zwp_input_panel_surface_v1 :: ZwpInputPanelSurfaceV1 > (_args [0] . n) . unwrap ()
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
    pub struct ZwpInputPanelV1(Resource<ZwpInputPanelV1>);
    impl AsRef<Resource<ZwpInputPanelV1>> for ZwpInputPanelV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwpInputPanelV1>> for ZwpInputPanelV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwpInputPanelV1(value)
        }
    }
    impl From<ZwpInputPanelV1> for Resource<ZwpInputPanelV1> {
        #[inline]
        fn from(value: ZwpInputPanelV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwpInputPanelV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwpInputPanelV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwp_input_panel_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwp_input_panel_v1_interface }
        }
    }
    impl ZwpInputPanelV1 {}
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_GET_INPUT_PANEL_SURFACE_SINCE: u32 = 1u32;
    static mut zwp_input_panel_v1_requests_get_input_panel_surface_types: [*const wl_interface; 2] = [
        unsafe {
            &super::zwp_input_panel_surface_v1::zwp_input_panel_surface_v1_interface
                as *const wl_interface
        },
        unsafe { &super::wl_surface::wl_surface_interface as *const wl_interface },
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_input_panel_v1_requests: [wl_message; 1] = [wl_message {
        name: b"get_input_panel_surface\0" as *const u8 as *const c_char,
        signature: b"no\0" as *const u8 as *const c_char,
        types: unsafe { &zwp_input_panel_v1_requests_get_input_panel_surface_types as *const _ },
    }];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwp_input_panel_v1_interface: wl_interface = wl_interface {
        name: b"zwp_input_panel_v1\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 1,
        requests: unsafe { &zwp_input_panel_v1_requests as *const _ },
        event_count: 0,
        events: NULLPTR as *const wl_message,
    };
}
pub mod zwp_input_panel_surface_v1 {
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
    pub enum Position {
        CenterBottom = 0,
    }
    impl Position {
        pub fn from_raw(n: u32) -> Option<Position> {
            match n {
                0 => Some(Position::CenterBottom),
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
        #[doc = "set the surface type as a keyboard\n\nSet the input_panel_surface type to keyboard.\n\nA keyboard surface is only shown when a text input is active."]
        SetToplevel {
            output: super::wl_output::WlOutput,
            position: u32,
        },
        #[doc = "set the surface type as an overlay panel\n\nSet the input_panel_surface to be an overlay panel.\n\nThis is shown near the input cursor above the application window when\na text input is active."]
        SetOverlayPanel,
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "set_toplevel",
                since: 1,
                signature: &[super::ArgumentType::Object, super::ArgumentType::Uint],
                destructor: false,
            },
            super::MessageDesc {
                name: "set_overlay_panel",
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
                Request::SetToplevel { .. } => 0,
                Request::SetOverlayPanel => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::SetToplevel { .. } => 1,
                Request::SetOverlayPanel => 1,
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
                    Ok(Request::SetToplevel {
                        output: {
                            if let Some(Argument::Object(val)) = args.next() {
                                map.get(val).ok_or(())?.into()
                            } else {
                                return Err(());
                            }
                        },
                        position: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                1 => Ok(Request::SetOverlayPanel),
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
                    Ok(Request::SetToplevel {
                        output: Resource::<super::wl_output::WlOutput>::from_c_ptr(
                            _args[0].o as *mut _,
                        )
                        .into(),
                        position: _args[1].u,
                    })
                }
                1 => Ok(Request::SetOverlayPanel),
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
    pub struct ZwpInputPanelSurfaceV1(Resource<ZwpInputPanelSurfaceV1>);
    impl AsRef<Resource<ZwpInputPanelSurfaceV1>> for ZwpInputPanelSurfaceV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwpInputPanelSurfaceV1>> for ZwpInputPanelSurfaceV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwpInputPanelSurfaceV1(value)
        }
    }
    impl From<ZwpInputPanelSurfaceV1> for Resource<ZwpInputPanelSurfaceV1> {
        #[inline]
        fn from(value: ZwpInputPanelSurfaceV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwpInputPanelSurfaceV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwpInputPanelSurfaceV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwp_input_panel_surface_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwp_input_panel_surface_v1_interface }
        }
    }
    impl ZwpInputPanelSurfaceV1 {}
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_SET_TOPLEVEL_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_SET_OVERLAY_PANEL_SINCE: u32 = 1u32;
    static mut zwp_input_panel_surface_v1_requests_set_toplevel_types: [*const wl_interface; 2] = [
        unsafe { &super::wl_output::wl_output_interface as *const wl_interface },
        NULLPTR as *const wl_interface,
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_input_panel_surface_v1_requests: [wl_message; 2] = [
        wl_message {
            name: b"set_toplevel\0" as *const u8 as *const c_char,
            signature: b"ou\0" as *const u8 as *const c_char,
            types: unsafe { &zwp_input_panel_surface_v1_requests_set_toplevel_types as *const _ },
        },
        wl_message {
            name: b"set_overlay_panel\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwp_input_panel_surface_v1_interface: wl_interface = wl_interface {
        name: b"zwp_input_panel_surface_v1\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 2,
        requests: unsafe { &zwp_input_panel_surface_v1_requests as *const _ },
        event_count: 0,
        events: NULLPTR as *const wl_message,
    };
}
