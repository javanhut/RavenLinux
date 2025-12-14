use std::os::raw::{c_char, c_void};
const NULLPTR: *const c_void = 0 as *const c_void;
static mut types_null: [*const sys::common::wl_interface; 5] = [
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
];
#[doc = "text input\n\nAn object used for text input. Adds support for text input and input\nmethods to applications. A text_input object is created from a\nwl_text_input_manager and corresponds typically to a text entry in an\napplication.\n\nRequests are used to activate/deactivate the text_input object and set\nstate information like surrounding and selected text or the content type.\nThe information about entered text is sent to the text_input object via\nthe pre-edit and commit events. Using this interface removes the need\nfor applications to directly process hardware key events and compose text\nout of them.\n\nText is generally UTF-8 encoded, indices and lengths are in bytes.\n\nSerials are used to synchronize the state between the text input and\nan input method. New serials are sent by the text input in the\ncommit_state request and are used by the input method to indicate\nthe known text input state in events like preedit_string, commit_string,\nand keysym. The text input can then ignore events from the input method\nwhich are based on an outdated state (for example after a reset).\n\nWarning! The protocol described in this file is experimental and\nbackward incompatible changes may be made. Backward compatible changes\nmay be added together with the corresponding interface version bump.\nBackward incompatible changes are done by bumping the version number in\nthe protocol and interface names and resetting the interface version.\nOnce the protocol is to be declared stable, the 'z' prefix and the\nversion number in the protocol and interface names are removed and the\ninterface version number is reset."]
pub mod zwp_text_input_v1 {
    use super::sys::common::{wl_argument, wl_array, wl_interface, wl_message};
    use super::sys::server::*;
    use super::{
        smallvec, types_null, AnonymousObject, Argument, ArgumentType, Interface, Main, Message,
        MessageDesc, MessageGroup, Object, ObjectMetadata, Resource, NULLPTR,
    };
    use std::os::raw::c_char;
    bitflags! { # [doc = "content hint\n\nContent hint is a bitmask to allow to modify the behavior of the text\ninput."] pub struct ContentHint : u32 { # [doc = "no special behaviour"] const None = 0 ; # [doc = "auto completion, correction and capitalization"] const Default = 7 ; # [doc = "hidden and sensitive text"] const Password = 192 ; # [doc = "suggest word completions"] const AutoCompletion = 1 ; # [doc = "suggest word corrections"] const AutoCorrection = 2 ; # [doc = "switch to uppercase letters at the start of a sentence"] const AutoCapitalization = 4 ; # [doc = "prefer lowercase letters"] const Lowercase = 8 ; # [doc = "prefer uppercase letters"] const Uppercase = 16 ; # [doc = "prefer casing for titles and headings (can be language dependent)"] const Titlecase = 32 ; # [doc = "characters should be hidden"] const HiddenText = 64 ; # [doc = "typed text should not be stored"] const SensitiveData = 128 ; # [doc = "just latin characters should be entered"] const Latin = 256 ; # [doc = "the text input is multiline"] const Multiline = 512 ; } }
    impl ContentHint {
        pub fn from_raw(n: u32) -> Option<ContentHint> {
            Some(ContentHint::from_bits_truncate(n))
        }
        pub fn to_raw(&self) -> u32 {
            self.bits()
        }
    }
    #[doc = "content purpose\n\nThe content purpose allows to specify the primary purpose of a text\ninput.\n\nThis allows an input method to show special purpose input panels with\nextra characters or to disallow some characters."]
    #[repr(u32)]
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[non_exhaustive]
    pub enum ContentPurpose {
        #[doc = "default input, allowing all characters"]
        Normal = 0,
        #[doc = "allow only alphabetic characters"]
        Alpha = 1,
        #[doc = "allow only digits"]
        Digits = 2,
        #[doc = "input a number (including decimal separator and sign)"]
        Number = 3,
        #[doc = "input a phone number"]
        Phone = 4,
        #[doc = "input an URL"]
        Url = 5,
        #[doc = "input an email address"]
        Email = 6,
        #[doc = "input a name of a person"]
        Name = 7,
        #[doc = "input a password (combine with password or sensitive_data hint)"]
        Password = 8,
        #[doc = "input a date"]
        Date = 9,
        #[doc = "input a time"]
        Time = 10,
        #[doc = "input a date and time"]
        Datetime = 11,
        #[doc = "input for a terminal"]
        Terminal = 12,
    }
    impl ContentPurpose {
        pub fn from_raw(n: u32) -> Option<ContentPurpose> {
            match n {
                0 => Some(ContentPurpose::Normal),
                1 => Some(ContentPurpose::Alpha),
                2 => Some(ContentPurpose::Digits),
                3 => Some(ContentPurpose::Number),
                4 => Some(ContentPurpose::Phone),
                5 => Some(ContentPurpose::Url),
                6 => Some(ContentPurpose::Email),
                7 => Some(ContentPurpose::Name),
                8 => Some(ContentPurpose::Password),
                9 => Some(ContentPurpose::Date),
                10 => Some(ContentPurpose::Time),
                11 => Some(ContentPurpose::Datetime),
                12 => Some(ContentPurpose::Terminal),
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
    pub enum PreeditStyle {
        #[doc = "default style for composing text"]
        Default = 0,
        #[doc = "style should be the same as in non-composing text"]
        None = 1,
        Active = 2,
        Inactive = 3,
        Highlight = 4,
        Underline = 5,
        Selection = 6,
        Incorrect = 7,
    }
    impl PreeditStyle {
        pub fn from_raw(n: u32) -> Option<PreeditStyle> {
            match n {
                0 => Some(PreeditStyle::Default),
                1 => Some(PreeditStyle::None),
                2 => Some(PreeditStyle::Active),
                3 => Some(PreeditStyle::Inactive),
                4 => Some(PreeditStyle::Highlight),
                5 => Some(PreeditStyle::Underline),
                6 => Some(PreeditStyle::Selection),
                7 => Some(PreeditStyle::Incorrect),
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
    pub enum TextDirection {
        #[doc = "automatic text direction based on text and language"]
        Auto = 0,
        #[doc = "left-to-right"]
        Ltr = 1,
        #[doc = "right-to-left"]
        Rtl = 2,
    }
    impl TextDirection {
        pub fn from_raw(n: u32) -> Option<TextDirection> {
            match n {
                0 => Some(TextDirection::Auto),
                1 => Some(TextDirection::Ltr),
                2 => Some(TextDirection::Rtl),
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
        #[doc = "request activation\n\nRequests the text_input object to be activated (typically when the\ntext entry gets focus).\n\nThe seat argument is a wl_seat which maintains the focus for this\nactivation. The surface argument is a wl_surface assigned to the\ntext_input object and tracked for focus lost. The enter event\nis emitted on successful activation."]
        Activate {
            seat: super::wl_seat::WlSeat,
            surface: super::wl_surface::WlSurface,
        },
        #[doc = "request deactivation\n\nRequests the text_input object to be deactivated (typically when the\ntext entry lost focus). The seat argument is a wl_seat which was used\nfor activation."]
        Deactivate { seat: super::wl_seat::WlSeat },
        #[doc = "show input panels\n\nRequests input panels (virtual keyboard) to show."]
        ShowInputPanel,
        #[doc = "hide input panels\n\nRequests input panels (virtual keyboard) to hide."]
        HideInputPanel,
        #[doc = "reset\n\nShould be called by an editor widget when the input state should be\nreset, for example after the text was changed outside of the normal\ninput method flow."]
        Reset,
        #[doc = "sets the surrounding text\n\nSets the plain surrounding text around the input position. Text is\nUTF-8 encoded. Cursor is the byte offset within the\nsurrounding text. Anchor is the byte offset of the\nselection anchor within the surrounding text. If there is no selected\ntext anchor, then it is the same as cursor."]
        SetSurroundingText {
            text: String,
            cursor: u32,
            anchor: u32,
        },
        #[doc = "set content purpose and hint\n\nSets the content purpose and content hint. While the purpose is the\nbasic purpose of an input field, the hint flags allow to modify some\nof the behavior.\n\nWhen no content type is explicitly set, a normal content purpose with\ndefault hints (auto completion, auto correction, auto capitalization)\nshould be assumed."]
        SetContentType {
            hint: ContentHint,
            purpose: ContentPurpose,
        },
        #[doc = ""]
        SetCursorRectangle {
            x: i32,
            y: i32,
            width: i32,
            height: i32,
        },
        #[doc = "sets preferred language\n\nSets a specific language. This allows for example a virtual keyboard to\nshow a language specific layout. The \"language\" argument is an RFC-3066\nformat language tag.\n\nIt could be used for example in a word processor to indicate the\nlanguage of the currently edited document or in an instant message\napplication which tracks languages of contacts."]
        SetPreferredLanguage { language: String },
        #[doc = ""]
        CommitState { serial: u32 },
        #[doc = ""]
        InvokeAction { button: u32, index: u32 },
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "activate",
                since: 1,
                signature: &[super::ArgumentType::Object, super::ArgumentType::Object],
                destructor: false,
            },
            super::MessageDesc {
                name: "deactivate",
                since: 1,
                signature: &[super::ArgumentType::Object],
                destructor: false,
            },
            super::MessageDesc {
                name: "show_input_panel",
                since: 1,
                signature: &[],
                destructor: false,
            },
            super::MessageDesc {
                name: "hide_input_panel",
                since: 1,
                signature: &[],
                destructor: false,
            },
            super::MessageDesc {
                name: "reset",
                since: 1,
                signature: &[],
                destructor: false,
            },
            super::MessageDesc {
                name: "set_surrounding_text",
                since: 1,
                signature: &[
                    super::ArgumentType::Str,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                ],
                destructor: false,
            },
            super::MessageDesc {
                name: "set_content_type",
                since: 1,
                signature: &[super::ArgumentType::Uint, super::ArgumentType::Uint],
                destructor: false,
            },
            super::MessageDesc {
                name: "set_cursor_rectangle",
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
                name: "set_preferred_language",
                since: 1,
                signature: &[super::ArgumentType::Str],
                destructor: false,
            },
            super::MessageDesc {
                name: "commit_state",
                since: 1,
                signature: &[super::ArgumentType::Uint],
                destructor: false,
            },
            super::MessageDesc {
                name: "invoke_action",
                since: 1,
                signature: &[super::ArgumentType::Uint, super::ArgumentType::Uint],
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
                Request::Activate { .. } => 0,
                Request::Deactivate { .. } => 1,
                Request::ShowInputPanel => 2,
                Request::HideInputPanel => 3,
                Request::Reset => 4,
                Request::SetSurroundingText { .. } => 5,
                Request::SetContentType { .. } => 6,
                Request::SetCursorRectangle { .. } => 7,
                Request::SetPreferredLanguage { .. } => 8,
                Request::CommitState { .. } => 9,
                Request::InvokeAction { .. } => 10,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::Activate { .. } => 1,
                Request::Deactivate { .. } => 1,
                Request::ShowInputPanel => 1,
                Request::HideInputPanel => 1,
                Request::Reset => 1,
                Request::SetSurroundingText { .. } => 1,
                Request::SetContentType { .. } => 1,
                Request::SetCursorRectangle { .. } => 1,
                Request::SetPreferredLanguage { .. } => 1,
                Request::CommitState { .. } => 1,
                Request::InvokeAction { .. } => 1,
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
                    Ok(Request::Activate {
                        seat: {
                            if let Some(Argument::Object(val)) = args.next() {
                                map.get(val).ok_or(())?.into()
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
                1 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::Deactivate {
                        seat: {
                            if let Some(Argument::Object(val)) = args.next() {
                                map.get(val).ok_or(())?.into()
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                2 => Ok(Request::ShowInputPanel),
                3 => Ok(Request::HideInputPanel),
                4 => Ok(Request::Reset),
                5 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::SetSurroundingText {
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
                        cursor: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        anchor: {
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
                    Ok(Request::SetContentType {
                        hint: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                ContentHint::from_raw(val).ok_or(())?
                            } else {
                                return Err(());
                            }
                        },
                        purpose: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                ContentPurpose::from_raw(val).ok_or(())?
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                7 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::SetCursorRectangle {
                        x: {
                            if let Some(Argument::Int(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        y: {
                            if let Some(Argument::Int(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        width: {
                            if let Some(Argument::Int(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        height: {
                            if let Some(Argument::Int(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                8 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::SetPreferredLanguage {
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
                9 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::CommitState {
                        serial: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                10 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::InvokeAction {
                        button: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        index: {
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
                0 => {
                    let _args = ::std::slice::from_raw_parts(args, 2);
                    Ok(Request::Activate {
                        seat: Resource::<super::wl_seat::WlSeat>::from_c_ptr(_args[0].o as *mut _)
                            .into(),
                        surface: Resource::<super::wl_surface::WlSurface>::from_c_ptr(
                            _args[1].o as *mut _,
                        )
                        .into(),
                    })
                }
                1 => {
                    let _args = ::std::slice::from_raw_parts(args, 1);
                    Ok(Request::Deactivate {
                        seat: Resource::<super::wl_seat::WlSeat>::from_c_ptr(_args[0].o as *mut _)
                            .into(),
                    })
                }
                2 => Ok(Request::ShowInputPanel),
                3 => Ok(Request::HideInputPanel),
                4 => Ok(Request::Reset),
                5 => {
                    let _args = ::std::slice::from_raw_parts(args, 3);
                    Ok(Request::SetSurroundingText {
                        text: ::std::ffi::CStr::from_ptr(_args[0].s)
                            .to_string_lossy()
                            .into_owned(),
                        cursor: _args[1].u,
                        anchor: _args[2].u,
                    })
                }
                6 => {
                    let _args = ::std::slice::from_raw_parts(args, 2);
                    Ok(Request::SetContentType {
                        hint: ContentHint::from_raw(_args[0].u).ok_or(())?,
                        purpose: ContentPurpose::from_raw(_args[1].u).ok_or(())?,
                    })
                }
                7 => {
                    let _args = ::std::slice::from_raw_parts(args, 4);
                    Ok(Request::SetCursorRectangle {
                        x: _args[0].i,
                        y: _args[1].i,
                        width: _args[2].i,
                        height: _args[3].i,
                    })
                }
                8 => {
                    let _args = ::std::slice::from_raw_parts(args, 1);
                    Ok(Request::SetPreferredLanguage {
                        language: ::std::ffi::CStr::from_ptr(_args[0].s)
                            .to_string_lossy()
                            .into_owned(),
                    })
                }
                9 => {
                    let _args = ::std::slice::from_raw_parts(args, 1);
                    Ok(Request::CommitState { serial: _args[0].u })
                }
                10 => {
                    let _args = ::std::slice::from_raw_parts(args, 2);
                    Ok(Request::InvokeAction {
                        button: _args[0].u,
                        index: _args[1].u,
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
        #[doc = "enter event\n\nNotify the text_input object when it received focus. Typically in\nresponse to an activate request."]
        Enter {
            surface: super::wl_surface::WlSurface,
        },
        #[doc = "leave event\n\nNotify the text_input object when it lost focus. Either in response\nto a deactivate request or when the assigned surface lost focus or was\ndestroyed."]
        Leave,
        #[doc = "modifiers map\n\nTransfer an array of 0-terminated modifier names. The position in\nthe array is the index of the modifier as used in the modifiers\nbitmask in the keysym event."]
        ModifiersMap { map: Vec<u8> },
        #[doc = "state of the input panel\n\nNotify when the visibility state of the input panel changed."]
        InputPanelState { state: u32 },
        #[doc = "pre-edit\n\nNotify when a new composing text (pre-edit) should be set around the\ncurrent cursor position. Any previously set composing text should\nbe removed.\n\nThe commit text can be used to replace the preedit text on reset\n(for example on unfocus).\n\nThe text input should also handle all preedit_style and preedit_cursor\nevents occurring directly before preedit_string."]
        PreeditString {
            serial: u32,
            text: String,
            commit: String,
        },
        #[doc = "pre-edit styling\n\nSets styling information on composing text. The style is applied for\nlength bytes from index relative to the beginning of the composing\ntext (as byte offset). Multiple styles can\nbe applied to a composing text by sending multiple preedit_styling\nevents.\n\nThis event is handled as part of a following preedit_string event."]
        PreeditStyling {
            index: u32,
            length: u32,
            style: PreeditStyle,
        },
        #[doc = "pre-edit cursor\n\nSets the cursor position inside the composing text (as byte\noffset) relative to the start of the composing text. When index is a\nnegative number no cursor is shown.\n\nThis event is handled as part of a following preedit_string event."]
        PreeditCursor { index: i32 },
        #[doc = "commit\n\nNotify when text should be inserted into the editor widget. The text to\ncommit could be either just a single character after a key press or the\nresult of some composing (pre-edit). It could also be an empty text\nwhen some text should be removed (see delete_surrounding_text) or when\nthe input cursor should be moved (see cursor_position).\n\nAny previously set composing text should be removed."]
        CommitString { serial: u32, text: String },
        #[doc = "set cursor to new position\n\nNotify when the cursor or anchor position should be modified.\n\nThis event should be handled as part of a following commit_string\nevent."]
        CursorPosition { index: i32, anchor: i32 },
        #[doc = "delete surrounding text\n\nNotify when the text around the current cursor position should be\ndeleted.\n\nIndex is relative to the current cursor (in bytes).\nLength is the length of deleted text (in bytes).\n\nThis event should be handled as part of a following commit_string\nevent."]
        DeleteSurroundingText { index: i32, length: u32 },
        #[doc = "keysym\n\nNotify when a key event was sent. Key events should not be used\nfor normal text input operations, which should be done with\ncommit_string, delete_surrounding_text, etc. The key event follows\nthe wl_keyboard key event convention. Sym is an XKB keysym, state a\nwl_keyboard key_state. Modifiers are a mask for effective modifiers\n(where the modifier indices are set by the modifiers_map event)"]
        Keysym {
            serial: u32,
            time: u32,
            sym: u32,
            state: u32,
            modifiers: u32,
        },
        #[doc = "language\n\nSets the language of the input text. The \"language\" argument is an\nRFC-3066 format language tag."]
        Language { serial: u32, language: String },
        #[doc = "text direction\n\nSets the text direction of input text.\n\nIt is mainly needed for showing an input cursor on the correct side of\nthe editor when there is no input done yet and making sure neutral\ndirection text is laid out properly."]
        TextDirection {
            serial: u32,
            direction: TextDirection,
        },
    }
    impl super::MessageGroup for Event {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "enter",
                since: 1,
                signature: &[super::ArgumentType::Object],
                destructor: false,
            },
            super::MessageDesc {
                name: "leave",
                since: 1,
                signature: &[],
                destructor: false,
            },
            super::MessageDesc {
                name: "modifiers_map",
                since: 1,
                signature: &[super::ArgumentType::Array],
                destructor: false,
            },
            super::MessageDesc {
                name: "input_panel_state",
                since: 1,
                signature: &[super::ArgumentType::Uint],
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
                name: "commit_string",
                since: 1,
                signature: &[super::ArgumentType::Uint, super::ArgumentType::Str],
                destructor: false,
            },
            super::MessageDesc {
                name: "cursor_position",
                since: 1,
                signature: &[super::ArgumentType::Int, super::ArgumentType::Int],
                destructor: false,
            },
            super::MessageDesc {
                name: "delete_surrounding_text",
                since: 1,
                signature: &[super::ArgumentType::Int, super::ArgumentType::Uint],
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
                _ => false,
            }
        }
        fn opcode(&self) -> u16 {
            match *self {
                Event::Enter { .. } => 0,
                Event::Leave => 1,
                Event::ModifiersMap { .. } => 2,
                Event::InputPanelState { .. } => 3,
                Event::PreeditString { .. } => 4,
                Event::PreeditStyling { .. } => 5,
                Event::PreeditCursor { .. } => 6,
                Event::CommitString { .. } => 7,
                Event::CursorPosition { .. } => 8,
                Event::DeleteSurroundingText { .. } => 9,
                Event::Keysym { .. } => 10,
                Event::Language { .. } => 11,
                Event::TextDirection { .. } => 12,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Event::Enter { .. } => 1,
                Event::Leave => 1,
                Event::ModifiersMap { .. } => 1,
                Event::InputPanelState { .. } => 1,
                Event::PreeditString { .. } => 1,
                Event::PreeditStyling { .. } => 1,
                Event::PreeditCursor { .. } => 1,
                Event::CommitString { .. } => 1,
                Event::CursorPosition { .. } => 1,
                Event::DeleteSurroundingText { .. } => 1,
                Event::Keysym { .. } => 1,
                Event::Language { .. } => 1,
                Event::TextDirection { .. } => 1,
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
                Event::Enter { surface } => Message {
                    sender_id: sender_id,
                    opcode: 0,
                    args: smallvec![Argument::Object(surface.as_ref().id()),],
                },
                Event::Leave => Message {
                    sender_id: sender_id,
                    opcode: 1,
                    args: smallvec![],
                },
                Event::ModifiersMap { map } => Message {
                    sender_id: sender_id,
                    opcode: 2,
                    args: smallvec![Argument::Array(Box::new(map)),],
                },
                Event::InputPanelState { state } => Message {
                    sender_id: sender_id,
                    opcode: 3,
                    args: smallvec![Argument::Uint(state),],
                },
                Event::PreeditString {
                    serial,
                    text,
                    commit,
                } => Message {
                    sender_id: sender_id,
                    opcode: 4,
                    args: smallvec![
                        Argument::Uint(serial),
                        Argument::Str(Box::new(unsafe {
                            ::std::ffi::CString::from_vec_unchecked(text.into())
                        })),
                        Argument::Str(Box::new(unsafe {
                            ::std::ffi::CString::from_vec_unchecked(commit.into())
                        })),
                    ],
                },
                Event::PreeditStyling {
                    index,
                    length,
                    style,
                } => Message {
                    sender_id: sender_id,
                    opcode: 5,
                    args: smallvec![
                        Argument::Uint(index),
                        Argument::Uint(length),
                        Argument::Uint(style.to_raw()),
                    ],
                },
                Event::PreeditCursor { index } => Message {
                    sender_id: sender_id,
                    opcode: 6,
                    args: smallvec![Argument::Int(index),],
                },
                Event::CommitString { serial, text } => Message {
                    sender_id: sender_id,
                    opcode: 7,
                    args: smallvec![
                        Argument::Uint(serial),
                        Argument::Str(Box::new(unsafe {
                            ::std::ffi::CString::from_vec_unchecked(text.into())
                        })),
                    ],
                },
                Event::CursorPosition { index, anchor } => Message {
                    sender_id: sender_id,
                    opcode: 8,
                    args: smallvec![Argument::Int(index), Argument::Int(anchor),],
                },
                Event::DeleteSurroundingText { index, length } => Message {
                    sender_id: sender_id,
                    opcode: 9,
                    args: smallvec![Argument::Int(index), Argument::Uint(length),],
                },
                Event::Keysym {
                    serial,
                    time,
                    sym,
                    state,
                    modifiers,
                } => Message {
                    sender_id: sender_id,
                    opcode: 10,
                    args: smallvec![
                        Argument::Uint(serial),
                        Argument::Uint(time),
                        Argument::Uint(sym),
                        Argument::Uint(state),
                        Argument::Uint(modifiers),
                    ],
                },
                Event::Language { serial, language } => Message {
                    sender_id: sender_id,
                    opcode: 11,
                    args: smallvec![
                        Argument::Uint(serial),
                        Argument::Str(Box::new(unsafe {
                            ::std::ffi::CString::from_vec_unchecked(language.into())
                        })),
                    ],
                },
                Event::TextDirection { serial, direction } => Message {
                    sender_id: sender_id,
                    opcode: 12,
                    args: smallvec![Argument::Uint(serial), Argument::Uint(direction.to_raw()),],
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
                Event::Enter { surface } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].o = surface.as_ref().c_ptr() as *mut _;
                    f(0, &mut _args_array)
                }
                Event::Leave => {
                    let mut _args_array: [wl_argument; 0] = unsafe { ::std::mem::zeroed() };
                    f(1, &mut _args_array)
                }
                Event::ModifiersMap { map } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    let _arg_0 = wl_array {
                        size: map.len(),
                        alloc: map.capacity(),
                        data: map.as_ptr() as *mut _,
                    };
                    _args_array[0].a = &_arg_0;
                    f(2, &mut _args_array)
                }
                Event::InputPanelState { state } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = state;
                    f(3, &mut _args_array)
                }
                Event::PreeditString {
                    serial,
                    text,
                    commit,
                } => {
                    let mut _args_array: [wl_argument; 3] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = serial;
                    let _arg_1 = ::std::ffi::CString::new(text).unwrap();
                    _args_array[1].s = _arg_1.as_ptr();
                    let _arg_2 = ::std::ffi::CString::new(commit).unwrap();
                    _args_array[2].s = _arg_2.as_ptr();
                    f(4, &mut _args_array)
                }
                Event::PreeditStyling {
                    index,
                    length,
                    style,
                } => {
                    let mut _args_array: [wl_argument; 3] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = index;
                    _args_array[1].u = length;
                    _args_array[2].u = style.to_raw();
                    f(5, &mut _args_array)
                }
                Event::PreeditCursor { index } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].i = index;
                    f(6, &mut _args_array)
                }
                Event::CommitString { serial, text } => {
                    let mut _args_array: [wl_argument; 2] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = serial;
                    let _arg_1 = ::std::ffi::CString::new(text).unwrap();
                    _args_array[1].s = _arg_1.as_ptr();
                    f(7, &mut _args_array)
                }
                Event::CursorPosition { index, anchor } => {
                    let mut _args_array: [wl_argument; 2] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].i = index;
                    _args_array[1].i = anchor;
                    f(8, &mut _args_array)
                }
                Event::DeleteSurroundingText { index, length } => {
                    let mut _args_array: [wl_argument; 2] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].i = index;
                    _args_array[1].u = length;
                    f(9, &mut _args_array)
                }
                Event::Keysym {
                    serial,
                    time,
                    sym,
                    state,
                    modifiers,
                } => {
                    let mut _args_array: [wl_argument; 5] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = serial;
                    _args_array[1].u = time;
                    _args_array[2].u = sym;
                    _args_array[3].u = state;
                    _args_array[4].u = modifiers;
                    f(10, &mut _args_array)
                }
                Event::Language { serial, language } => {
                    let mut _args_array: [wl_argument; 2] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = serial;
                    let _arg_1 = ::std::ffi::CString::new(language).unwrap();
                    _args_array[1].s = _arg_1.as_ptr();
                    f(11, &mut _args_array)
                }
                Event::TextDirection { serial, direction } => {
                    let mut _args_array: [wl_argument; 2] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = serial;
                    _args_array[1].u = direction.to_raw();
                    f(12, &mut _args_array)
                }
            }
        }
    }
    #[derive(Clone, Eq, PartialEq)]
    pub struct ZwpTextInputV1(Resource<ZwpTextInputV1>);
    impl AsRef<Resource<ZwpTextInputV1>> for ZwpTextInputV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwpTextInputV1>> for ZwpTextInputV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwpTextInputV1(value)
        }
    }
    impl From<ZwpTextInputV1> for Resource<ZwpTextInputV1> {
        #[inline]
        fn from(value: ZwpTextInputV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwpTextInputV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwpTextInputV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwp_text_input_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwp_text_input_v1_interface }
        }
    }
    impl ZwpTextInputV1 {
        #[doc = "enter event\n\nNotify the text_input object when it received focus. Typically in\nresponse to an activate request."]
        pub fn enter(&self, surface: &super::wl_surface::WlSurface) -> () {
            let msg = Event::Enter {
                surface: surface.clone(),
            };
            self.0.send(msg);
        }
        #[doc = "leave event\n\nNotify the text_input object when it lost focus. Either in response\nto a deactivate request or when the assigned surface lost focus or was\ndestroyed."]
        pub fn leave(&self) -> () {
            let msg = Event::Leave;
            self.0.send(msg);
        }
        #[doc = "modifiers map\n\nTransfer an array of 0-terminated modifier names. The position in\nthe array is the index of the modifier as used in the modifiers\nbitmask in the keysym event."]
        pub fn modifiers_map(&self, map: Vec<u8>) -> () {
            let msg = Event::ModifiersMap { map: map };
            self.0.send(msg);
        }
        #[doc = "state of the input panel\n\nNotify when the visibility state of the input panel changed."]
        pub fn input_panel_state(&self, state: u32) -> () {
            let msg = Event::InputPanelState { state: state };
            self.0.send(msg);
        }
        #[doc = "pre-edit\n\nNotify when a new composing text (pre-edit) should be set around the\ncurrent cursor position. Any previously set composing text should\nbe removed.\n\nThe commit text can be used to replace the preedit text on reset\n(for example on unfocus).\n\nThe text input should also handle all preedit_style and preedit_cursor\nevents occurring directly before preedit_string."]
        pub fn preedit_string(&self, serial: u32, text: String, commit: String) -> () {
            let msg = Event::PreeditString {
                serial: serial,
                text: text,
                commit: commit,
            };
            self.0.send(msg);
        }
        #[doc = "pre-edit styling\n\nSets styling information on composing text. The style is applied for\nlength bytes from index relative to the beginning of the composing\ntext (as byte offset). Multiple styles can\nbe applied to a composing text by sending multiple preedit_styling\nevents.\n\nThis event is handled as part of a following preedit_string event."]
        pub fn preedit_styling(&self, index: u32, length: u32, style: PreeditStyle) -> () {
            let msg = Event::PreeditStyling {
                index: index,
                length: length,
                style: style,
            };
            self.0.send(msg);
        }
        #[doc = "pre-edit cursor\n\nSets the cursor position inside the composing text (as byte\noffset) relative to the start of the composing text. When index is a\nnegative number no cursor is shown.\n\nThis event is handled as part of a following preedit_string event."]
        pub fn preedit_cursor(&self, index: i32) -> () {
            let msg = Event::PreeditCursor { index: index };
            self.0.send(msg);
        }
        #[doc = "commit\n\nNotify when text should be inserted into the editor widget. The text to\ncommit could be either just a single character after a key press or the\nresult of some composing (pre-edit). It could also be an empty text\nwhen some text should be removed (see delete_surrounding_text) or when\nthe input cursor should be moved (see cursor_position).\n\nAny previously set composing text should be removed."]
        pub fn commit_string(&self, serial: u32, text: String) -> () {
            let msg = Event::CommitString {
                serial: serial,
                text: text,
            };
            self.0.send(msg);
        }
        #[doc = "set cursor to new position\n\nNotify when the cursor or anchor position should be modified.\n\nThis event should be handled as part of a following commit_string\nevent."]
        pub fn cursor_position(&self, index: i32, anchor: i32) -> () {
            let msg = Event::CursorPosition {
                index: index,
                anchor: anchor,
            };
            self.0.send(msg);
        }
        #[doc = "delete surrounding text\n\nNotify when the text around the current cursor position should be\ndeleted.\n\nIndex is relative to the current cursor (in bytes).\nLength is the length of deleted text (in bytes).\n\nThis event should be handled as part of a following commit_string\nevent."]
        pub fn delete_surrounding_text(&self, index: i32, length: u32) -> () {
            let msg = Event::DeleteSurroundingText {
                index: index,
                length: length,
            };
            self.0.send(msg);
        }
        #[doc = "keysym\n\nNotify when a key event was sent. Key events should not be used\nfor normal text input operations, which should be done with\ncommit_string, delete_surrounding_text, etc. The key event follows\nthe wl_keyboard key event convention. Sym is an XKB keysym, state a\nwl_keyboard key_state. Modifiers are a mask for effective modifiers\n(where the modifier indices are set by the modifiers_map event)"]
        pub fn keysym(&self, serial: u32, time: u32, sym: u32, state: u32, modifiers: u32) -> () {
            let msg = Event::Keysym {
                serial: serial,
                time: time,
                sym: sym,
                state: state,
                modifiers: modifiers,
            };
            self.0.send(msg);
        }
        #[doc = "language\n\nSets the language of the input text. The \"language\" argument is an\nRFC-3066 format language tag."]
        pub fn language(&self, serial: u32, language: String) -> () {
            let msg = Event::Language {
                serial: serial,
                language: language,
            };
            self.0.send(msg);
        }
        #[doc = "text direction\n\nSets the text direction of input text.\n\nIt is mainly needed for showing an input cursor on the correct side of\nthe editor when there is no input done yet and making sure neutral\ndirection text is laid out properly."]
        pub fn text_direction(&self, serial: u32, direction: TextDirection) -> () {
            let msg = Event::TextDirection {
                serial: serial,
                direction: direction,
            };
            self.0.send(msg);
        }
    }
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_ACTIVATE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DEACTIVATE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_SHOW_INPUT_PANEL_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_HIDE_INPUT_PANEL_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_RESET_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_SET_SURROUNDING_TEXT_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_SET_CONTENT_TYPE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_SET_CURSOR_RECTANGLE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_SET_PREFERRED_LANGUAGE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_COMMIT_STATE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_INVOKE_ACTION_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_ENTER_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_LEAVE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_MODIFIERS_MAP_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_INPUT_PANEL_STATE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_PREEDIT_STRING_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_PREEDIT_STYLING_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_PREEDIT_CURSOR_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_COMMIT_STRING_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_CURSOR_POSITION_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_DELETE_SURROUNDING_TEXT_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_KEYSYM_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_LANGUAGE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_TEXT_DIRECTION_SINCE: u32 = 1u32;
    static mut zwp_text_input_v1_requests_activate_types: [*const wl_interface; 2] = [
        unsafe { &super::wl_seat::wl_seat_interface as *const wl_interface },
        unsafe { &super::wl_surface::wl_surface_interface as *const wl_interface },
    ];
    static mut zwp_text_input_v1_requests_deactivate_types: [*const wl_interface; 1] =
        [unsafe { &super::wl_seat::wl_seat_interface as *const wl_interface }];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_text_input_v1_requests: [wl_message; 11] = [
        wl_message {
            name: b"activate\0" as *const u8 as *const c_char,
            signature: b"oo\0" as *const u8 as *const c_char,
            types: unsafe { &zwp_text_input_v1_requests_activate_types as *const _ },
        },
        wl_message {
            name: b"deactivate\0" as *const u8 as *const c_char,
            signature: b"o\0" as *const u8 as *const c_char,
            types: unsafe { &zwp_text_input_v1_requests_deactivate_types as *const _ },
        },
        wl_message {
            name: b"show_input_panel\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"hide_input_panel\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"reset\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"set_surrounding_text\0" as *const u8 as *const c_char,
            signature: b"suu\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"set_content_type\0" as *const u8 as *const c_char,
            signature: b"uu\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"set_cursor_rectangle\0" as *const u8 as *const c_char,
            signature: b"iiii\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"set_preferred_language\0" as *const u8 as *const c_char,
            signature: b"s\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"commit_state\0" as *const u8 as *const c_char,
            signature: b"u\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"invoke_action\0" as *const u8 as *const c_char,
            signature: b"uu\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    static mut zwp_text_input_v1_events_enter_types: [*const wl_interface; 1] =
        [unsafe { &super::wl_surface::wl_surface_interface as *const wl_interface }];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_text_input_v1_events: [wl_message; 13] = [
        wl_message {
            name: b"enter\0" as *const u8 as *const c_char,
            signature: b"o\0" as *const u8 as *const c_char,
            types: unsafe { &zwp_text_input_v1_events_enter_types as *const _ },
        },
        wl_message {
            name: b"leave\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"modifiers_map\0" as *const u8 as *const c_char,
            signature: b"a\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"input_panel_state\0" as *const u8 as *const c_char,
            signature: b"u\0" as *const u8 as *const c_char,
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
            name: b"commit_string\0" as *const u8 as *const c_char,
            signature: b"us\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"cursor_position\0" as *const u8 as *const c_char,
            signature: b"ii\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"delete_surrounding_text\0" as *const u8 as *const c_char,
            signature: b"iu\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"keysym\0" as *const u8 as *const c_char,
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
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwp_text_input_v1_interface: wl_interface = wl_interface {
        name: b"zwp_text_input_v1\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 11,
        requests: unsafe { &zwp_text_input_v1_requests as *const _ },
        event_count: 13,
        events: unsafe { &zwp_text_input_v1_events as *const _ },
    };
}
#[doc = "text input manager\n\nA factory for text_input objects. This object is a global singleton."]
pub mod zwp_text_input_manager_v1 {
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
        #[doc = "create text input\n\nCreates a new text_input object."]
        CreateTextInput {
            id: Main<super::zwp_text_input_v1::ZwpTextInputV1>,
        },
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[super::MessageDesc {
            name: "create_text_input",
            since: 1,
            signature: &[super::ArgumentType::NewId],
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
                Request::CreateTextInput { .. } => 0,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::CreateTextInput { .. } => 1,
            }
        }
        fn child<Meta: ObjectMetadata>(
            opcode: u16,
            version: u32,
            meta: &Meta,
        ) -> Option<Object<Meta>> {
            match opcode {
                0 => Some(Object::from_interface::<
                    super::zwp_text_input_v1::ZwpTextInputV1,
                >(version, meta.child())),
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            match msg.opcode {
                0 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::CreateTextInput {
                        id: {
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
                0 => {
                    let _args = ::std::slice::from_raw_parts(args, 1);
                    Ok(Request::CreateTextInput {
                        id: {
                            let me = Resource::<ZwpTextInputManagerV1>::from_c_ptr(obj as *mut _);
                            me.make_child_for::<super::zwp_text_input_v1::ZwpTextInputV1>(
                                _args[0].n,
                            )
                            .unwrap()
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
    pub struct ZwpTextInputManagerV1(Resource<ZwpTextInputManagerV1>);
    impl AsRef<Resource<ZwpTextInputManagerV1>> for ZwpTextInputManagerV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwpTextInputManagerV1>> for ZwpTextInputManagerV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwpTextInputManagerV1(value)
        }
    }
    impl From<ZwpTextInputManagerV1> for Resource<ZwpTextInputManagerV1> {
        #[inline]
        fn from(value: ZwpTextInputManagerV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwpTextInputManagerV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwpTextInputManagerV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwp_text_input_manager_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwp_text_input_manager_v1_interface }
        }
    }
    impl ZwpTextInputManagerV1 {}
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_CREATE_TEXT_INPUT_SINCE: u32 = 1u32;
    static mut zwp_text_input_manager_v1_requests_create_text_input_types: [*const wl_interface;
        1] =
        [
            unsafe {
                &super::zwp_text_input_v1::zwp_text_input_v1_interface as *const wl_interface
            },
        ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_text_input_manager_v1_requests: [wl_message; 1] = [wl_message {
        name: b"create_text_input\0" as *const u8 as *const c_char,
        signature: b"n\0" as *const u8 as *const c_char,
        types: unsafe { &zwp_text_input_manager_v1_requests_create_text_input_types as *const _ },
    }];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwp_text_input_manager_v1_interface: wl_interface = wl_interface {
        name: b"zwp_text_input_manager_v1\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 1,
        requests: unsafe { &zwp_text_input_manager_v1_requests as *const _ },
        event_count: 0,
        events: NULLPTR as *const wl_message,
    };
}
