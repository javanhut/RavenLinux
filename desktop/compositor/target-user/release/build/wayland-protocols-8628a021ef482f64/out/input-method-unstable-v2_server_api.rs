use std::os::raw::{c_char, c_void};
const NULLPTR: *const c_void = 0 as *const c_void;
static mut types_null: [*const sys::common::wl_interface; 5] = [
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
];
#[doc = "input method\n\nAn input method object allows for clients to compose text.\n\nThe objects connects the client to a text input in an application, and\nlets the client to serve as an input method for a seat.\n\nThe zwp_input_method_v2 object can occupy two distinct states: active and\ninactive. In the active state, the object is associated to and\ncommunicates with a text input. In the inactive state, there is no\nassociated text input, and the only communication is with the compositor.\nInitially, the input method is in the inactive state.\n\nRequests issued in the inactive state must be accepted by the compositor.\nBecause of the serial mechanism, and the state reset on activate event,\nthey will not have any effect on the state of the next text input.\n\nThere must be no more than one input method object per seat."]
pub mod zwp_input_method_v2 {
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
        #[doc = "commit string\n\nSend the commit string text for insertion to the application.\n\nInserts a string at current cursor position (see commit event\nsequence). The string to commit could be either just a single character\nafter a key press or the result of some composing.\n\nThe argument text is a buffer containing the string to insert. There is\na maximum length of wayland messages, so text can not be longer than\n4000 bytes.\n\nValues set with this event are double-buffered. They must be applied\nand reset to initial on the next zwp_text_input_v3.commit request.\n\nThe initial value of text is an empty string."]
        CommitString { text: String },
        #[doc = "pre-edit string\n\nSend the pre-edit string text to the application text input.\n\nPlace a new composing text (pre-edit) at the current cursor position.\nAny previously set composing text must be removed. Any previously\nexisting selected text must be removed. The cursor is moved to a new\nposition within the preedit string.\n\nThe argument text is a buffer containing the preedit string. There is\na maximum length of wayland messages, so text can not be longer than\n4000 bytes.\n\nThe arguments cursor_begin and cursor_end are counted in bytes relative\nto the beginning of the submitted string buffer. Cursor should be\nhidden by the text input when both are equal to -1.\n\ncursor_begin indicates the beginning of the cursor. cursor_end\nindicates the end of the cursor. It may be equal or different than\ncursor_begin.\n\nValues set with this event are double-buffered. They must be applied on\nthe next zwp_input_method_v2.commit event.\n\nThe initial value of text is an empty string. The initial value of\ncursor_begin, and cursor_end are both 0."]
        SetPreeditString {
            text: String,
            cursor_begin: i32,
            cursor_end: i32,
        },
        #[doc = "delete text\n\nRemove the surrounding text.\n\nbefore_length and after_length are the number of bytes before and after\nthe current cursor index (excluding the preedit text) to delete.\n\nIf any preedit text is present, it is replaced with the cursor for the\npurpose of this event. In effect before_length is counted from the\nbeginning of preedit text, and after_length from its end (see commit\nevent sequence).\n\nValues set with this event are double-buffered. They must be applied\nand reset to initial on the next zwp_input_method_v2.commit request.\n\nThe initial values of both before_length and after_length are 0."]
        DeleteSurroundingText {
            before_length: u32,
            after_length: u32,
        },
        #[doc = "apply state\n\nApply state changes from commit_string, set_preedit_string and\ndelete_surrounding_text requests.\n\nThe state relating to these events is double-buffered, and each one\nmodifies the pending state. This request replaces the current state\nwith the pending state.\n\nThe connected text input is expected to proceed by evaluating the\nchanges in the following order:\n\n1. Replace existing preedit string with the cursor.\n2. Delete requested surrounding text.\n3. Insert commit string with the cursor at its end.\n4. Calculate surrounding text to send.\n5. Insert new preedit text in cursor position.\n6. Place cursor inside preedit text.\n\nThe serial number reflects the last state of the zwp_input_method_v2\nobject known to the client. The value of the serial argument must be\nequal to the number of done events already issued by that object. When\nthe compositor receives a commit request with a serial different than\nthe number of past done events, it must proceed as normal, except it\nshould not change the current state of the zwp_input_method_v2 object."]
        Commit { serial: u32 },
        #[doc = "create popup surface\n\nCreates a new zwp_input_popup_surface_v2 object wrapping a given\nsurface.\n\nThe surface gets assigned the \"input_popup\" role. If the surface\nalready has an assigned role, the compositor must issue a protocol\nerror."]
        GetInputPopupSurface {
            id: Main<super::zwp_input_popup_surface_v2::ZwpInputPopupSurfaceV2>,
            surface: super::wl_surface::WlSurface,
        },
        #[doc = "grab hardware keyboard\n\nAllow an input method to receive hardware keyboard input and process\nkey events to generate text events (with pre-edit) over the wire. This\nallows input methods which compose multiple key events for inputting\ntext like it is done for CJK languages.\n\nThe compositor should send all keyboard events on the seat to the grab\nholder via the returned wl_keyboard object. Nevertheless, the\ncompositor may decide not to forward any particular event. The\ncompositor must not further process any event after it has been\nforwarded to the grab holder.\n\nReleasing the resulting wl_keyboard object releases the grab."]
        GrabKeyboard {
            keyboard: Main<super::zwp_input_method_keyboard_grab_v2::ZwpInputMethodKeyboardGrabV2>,
        },
        #[doc = "destroy the text input\n\nDestroys the zwp_text_input_v2 object and any associated child\nobjects, i.e. zwp_input_popup_surface_v2 and\nzwp_input_method_keyboard_grab_v2.\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "commit_string",
                since: 1,
                signature: &[super::ArgumentType::Str],
                destructor: false,
            },
            super::MessageDesc {
                name: "set_preedit_string",
                since: 1,
                signature: &[
                    super::ArgumentType::Str,
                    super::ArgumentType::Int,
                    super::ArgumentType::Int,
                ],
                destructor: false,
            },
            super::MessageDesc {
                name: "delete_surrounding_text",
                since: 1,
                signature: &[super::ArgumentType::Uint, super::ArgumentType::Uint],
                destructor: false,
            },
            super::MessageDesc {
                name: "commit",
                since: 1,
                signature: &[super::ArgumentType::Uint],
                destructor: false,
            },
            super::MessageDesc {
                name: "get_input_popup_surface",
                since: 1,
                signature: &[super::ArgumentType::NewId, super::ArgumentType::Object],
                destructor: false,
            },
            super::MessageDesc {
                name: "grab_keyboard",
                since: 1,
                signature: &[super::ArgumentType::NewId],
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
                Request::CommitString { .. } => 0,
                Request::SetPreeditString { .. } => 1,
                Request::DeleteSurroundingText { .. } => 2,
                Request::Commit { .. } => 3,
                Request::GetInputPopupSurface { .. } => 4,
                Request::GrabKeyboard { .. } => 5,
                Request::Destroy => 6,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::CommitString { .. } => 1,
                Request::SetPreeditString { .. } => 1,
                Request::DeleteSurroundingText { .. } => 1,
                Request::Commit { .. } => 1,
                Request::GetInputPopupSurface { .. } => 1,
                Request::GrabKeyboard { .. } => 1,
                Request::Destroy => 1,
            }
        }
        fn child<Meta: ObjectMetadata>(
            opcode: u16,
            version: u32,
            meta: &Meta,
        ) -> Option<Object<Meta>> {
            match opcode {
                4 => Some(Object::from_interface::<
                    super::zwp_input_popup_surface_v2::ZwpInputPopupSurfaceV2,
                >(version, meta.child())),
                5 => Some(Object::from_interface::<
                    super::zwp_input_method_keyboard_grab_v2::ZwpInputMethodKeyboardGrabV2,
                >(version, meta.child())),
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            match msg.opcode {
                0 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::CommitString {
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
                1 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::SetPreeditString {
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
                        cursor_begin: {
                            if let Some(Argument::Int(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        cursor_end: {
                            if let Some(Argument::Int(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                2 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::DeleteSurroundingText {
                        before_length: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        after_length: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                3 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::Commit {
                        serial: {
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
                    Ok(Request::GetInputPopupSurface {
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
                5 => {
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
                6 => Ok(Request::Destroy),
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
                    Ok(Request::CommitString {
                        text: ::std::ffi::CStr::from_ptr(_args[0].s)
                            .to_string_lossy()
                            .into_owned(),
                    })
                }
                1 => {
                    let _args = ::std::slice::from_raw_parts(args, 3);
                    Ok(Request::SetPreeditString {
                        text: ::std::ffi::CStr::from_ptr(_args[0].s)
                            .to_string_lossy()
                            .into_owned(),
                        cursor_begin: _args[1].i,
                        cursor_end: _args[2].i,
                    })
                }
                2 => {
                    let _args = ::std::slice::from_raw_parts(args, 2);
                    Ok(Request::DeleteSurroundingText {
                        before_length: _args[0].u,
                        after_length: _args[1].u,
                    })
                }
                3 => {
                    let _args = ::std::slice::from_raw_parts(args, 1);
                    Ok(Request::Commit { serial: _args[0].u })
                }
                4 => {
                    let _args = ::std::slice::from_raw_parts(args, 2);
                    Ok(Request::GetInputPopupSurface {
                        id: {
                            let me = Resource::<ZwpInputMethodV2>::from_c_ptr(obj as *mut _);
                            me . make_child_for :: < super :: zwp_input_popup_surface_v2 :: ZwpInputPopupSurfaceV2 > (_args [0] . n) . unwrap ()
                        },
                        surface: Resource::<super::wl_surface::WlSurface>::from_c_ptr(
                            _args[1].o as *mut _,
                        )
                        .into(),
                    })
                }
                5 => {
                    let _args = ::std::slice::from_raw_parts(args, 1);
                    Ok(Request::GrabKeyboard {
                        keyboard: {
                            let me = Resource::<ZwpInputMethodV2>::from_c_ptr(obj as *mut _);
                            me . make_child_for :: < super :: zwp_input_method_keyboard_grab_v2 :: ZwpInputMethodKeyboardGrabV2 > (_args [0] . n) . unwrap ()
                        },
                    })
                }
                6 => Ok(Request::Destroy),
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
        #[doc = "input method has been requested\n\nNotification that a text input focused on this seat requested the input\nmethod to be activated.\n\nThis event serves the purpose of providing the compositor with an\nactive input method.\n\nThis event resets all state associated with previous enable, disable,\nsurrounding_text, text_change_cause, and content_type events, as well\nas the state associated with set_preedit_string, commit_string, and\ndelete_surrounding_text requests. In addition, it marks the\nzwp_input_method_v2 object as active, and makes any existing\nzwp_input_popup_surface_v2 objects visible.\n\nThe surrounding_text, and content_type events must follow before the\nnext done event if the text input supports the respective\nfunctionality.\n\nState set with this event is double-buffered. It will get applied on\nthe next zwp_input_method_v2.done event, and stay valid until changed."]
        Activate,
        #[doc = "deactivate event\n\nNotification that no focused text input currently needs an active\ninput method on this seat.\n\nThis event marks the zwp_input_method_v2 object as inactive. The\ncompositor must make all existing zwp_input_popup_surface_v2 objects\ninvisible until the next activate event.\n\nState set with this event is double-buffered. It will get applied on\nthe next zwp_input_method_v2.done event, and stay valid until changed."]
        Deactivate,
        #[doc = "surrounding text event\n\nUpdates the surrounding plain text around the cursor, excluding the\npreedit text.\n\nIf any preedit text is present, it is replaced with the cursor for the\npurpose of this event.\n\nThe argument text is a buffer containing the preedit string, and must\ninclude the cursor position, and the complete selection. It should\ncontain additional characters before and after these. There is a\nmaximum length of wayland messages, so text can not be longer than 4000\nbytes.\n\ncursor is the byte offset of the cursor within the text buffer.\n\nanchor is the byte offset of the selection anchor within the text\nbuffer. If there is no selected text, anchor must be the same as\ncursor.\n\nIf this event does not arrive before the first done event, the input\nmethod may assume that the text input does not support this\nfunctionality and ignore following surrounding_text events.\n\nValues set with this event are double-buffered. They will get applied\nand set to initial values on the next zwp_input_method_v2.done\nevent.\n\nThe initial state for affected fields is empty, meaning that the text\ninput does not support sending surrounding text. If the empty values\nget applied, subsequent attempts to change them may have no effect."]
        SurroundingText {
            text: String,
            cursor: u32,
            anchor: u32,
        },
        #[doc = "indicates the cause of surrounding text change\n\nTells the input method why the text surrounding the cursor changed.\n\nWhenever the client detects an external change in text, cursor, or\nanchor position, it must issue this request to the compositor. This\nrequest is intended to give the input method a chance to update the\npreedit text in an appropriate way, e.g. by removing it when the user\nstarts typing with a keyboard.\n\ncause describes the source of the change.\n\nThe value set with this event is double-buffered. It will get applied\nand set to its initial value on the next zwp_input_method_v2.done\nevent.\n\nThe initial value of cause is input_method."]
        TextChangeCause {
            cause: super::zwp_text_input_v3::ChangeCause,
        },
        #[doc = "content purpose and hint\n\nIndicates the content type and hint for the current\nzwp_input_method_v2 instance.\n\nValues set with this event are double-buffered. They will get applied\non the next zwp_input_method_v2.done event.\n\nThe initial value for hint is none, and the initial value for purpose\nis normal."]
        ContentType {
            hint: super::zwp_text_input_v3::ContentHint,
            purpose: super::zwp_text_input_v3::ContentPurpose,
        },
        #[doc = "apply state\n\nAtomically applies state changes recently sent to the client.\n\nThe done event establishes and updates the state of the client, and\nmust be issued after any changes to apply them.\n\nText input state (content purpose, content hint, surrounding text, and\nchange cause) is conceptually double-buffered within an input method\ncontext.\n\nEvents modify the pending state, as opposed to the current state in use\nby the input method. A done event atomically applies all pending state,\nreplacing the current state. After done, the new pending state is as\ndocumented for each related request.\n\nEvents must be applied in the order of arrival.\n\nNeither current nor pending state are modified unless noted otherwise."]
        Done,
        #[doc = "input method unavailable\n\nThe input method ceased to be available.\n\nThe compositor must issue this event as the only event on the object if\nthere was another input_method object associated with the same seat at\nthe time of its creation.\n\nThe compositor must issue this request when the object is no longer\nuseable, e.g. due to seat removal.\n\nThe input method context becomes inert and should be destroyed after\ndeactivation is handled. Any further requests and events except for the\ndestroy request must be ignored."]
        Unavailable,
    }
    impl super::MessageGroup for Event {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "activate",
                since: 1,
                signature: &[],
                destructor: false,
            },
            super::MessageDesc {
                name: "deactivate",
                since: 1,
                signature: &[],
                destructor: false,
            },
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
                name: "text_change_cause",
                since: 1,
                signature: &[super::ArgumentType::Uint],
                destructor: false,
            },
            super::MessageDesc {
                name: "content_type",
                since: 1,
                signature: &[super::ArgumentType::Uint, super::ArgumentType::Uint],
                destructor: false,
            },
            super::MessageDesc {
                name: "done",
                since: 1,
                signature: &[],
                destructor: false,
            },
            super::MessageDesc {
                name: "unavailable",
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
                Event::Activate => 0,
                Event::Deactivate => 1,
                Event::SurroundingText { .. } => 2,
                Event::TextChangeCause { .. } => 3,
                Event::ContentType { .. } => 4,
                Event::Done => 5,
                Event::Unavailable => 6,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Event::Activate => 1,
                Event::Deactivate => 1,
                Event::SurroundingText { .. } => 1,
                Event::TextChangeCause { .. } => 1,
                Event::ContentType { .. } => 1,
                Event::Done => 1,
                Event::Unavailable => 1,
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
                Event::Activate => Message {
                    sender_id: sender_id,
                    opcode: 0,
                    args: smallvec![],
                },
                Event::Deactivate => Message {
                    sender_id: sender_id,
                    opcode: 1,
                    args: smallvec![],
                },
                Event::SurroundingText {
                    text,
                    cursor,
                    anchor,
                } => Message {
                    sender_id: sender_id,
                    opcode: 2,
                    args: smallvec![
                        Argument::Str(Box::new(unsafe {
                            ::std::ffi::CString::from_vec_unchecked(text.into())
                        })),
                        Argument::Uint(cursor),
                        Argument::Uint(anchor),
                    ],
                },
                Event::TextChangeCause { cause } => Message {
                    sender_id: sender_id,
                    opcode: 3,
                    args: smallvec![Argument::Uint(cause.to_raw()),],
                },
                Event::ContentType { hint, purpose } => Message {
                    sender_id: sender_id,
                    opcode: 4,
                    args: smallvec![
                        Argument::Uint(hint.to_raw()),
                        Argument::Uint(purpose.to_raw()),
                    ],
                },
                Event::Done => Message {
                    sender_id: sender_id,
                    opcode: 5,
                    args: smallvec![],
                },
                Event::Unavailable => Message {
                    sender_id: sender_id,
                    opcode: 6,
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
                Event::Activate => {
                    let mut _args_array: [wl_argument; 0] = unsafe { ::std::mem::zeroed() };
                    f(0, &mut _args_array)
                }
                Event::Deactivate => {
                    let mut _args_array: [wl_argument; 0] = unsafe { ::std::mem::zeroed() };
                    f(1, &mut _args_array)
                }
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
                    f(2, &mut _args_array)
                }
                Event::TextChangeCause { cause } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = cause.to_raw();
                    f(3, &mut _args_array)
                }
                Event::ContentType { hint, purpose } => {
                    let mut _args_array: [wl_argument; 2] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = hint.to_raw();
                    _args_array[1].u = purpose.to_raw();
                    f(4, &mut _args_array)
                }
                Event::Done => {
                    let mut _args_array: [wl_argument; 0] = unsafe { ::std::mem::zeroed() };
                    f(5, &mut _args_array)
                }
                Event::Unavailable => {
                    let mut _args_array: [wl_argument; 0] = unsafe { ::std::mem::zeroed() };
                    f(6, &mut _args_array)
                }
            }
        }
    }
    #[derive(Clone, Eq, PartialEq)]
    pub struct ZwpInputMethodV2(Resource<ZwpInputMethodV2>);
    impl AsRef<Resource<ZwpInputMethodV2>> for ZwpInputMethodV2 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwpInputMethodV2>> for ZwpInputMethodV2 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwpInputMethodV2(value)
        }
    }
    impl From<ZwpInputMethodV2> for Resource<ZwpInputMethodV2> {
        #[inline]
        fn from(value: ZwpInputMethodV2) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwpInputMethodV2 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwpInputMethodV2 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwp_input_method_v2";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwp_input_method_v2_interface }
        }
    }
    impl ZwpInputMethodV2 {
        #[doc = "input method has been requested\n\nNotification that a text input focused on this seat requested the input\nmethod to be activated.\n\nThis event serves the purpose of providing the compositor with an\nactive input method.\n\nThis event resets all state associated with previous enable, disable,\nsurrounding_text, text_change_cause, and content_type events, as well\nas the state associated with set_preedit_string, commit_string, and\ndelete_surrounding_text requests. In addition, it marks the\nzwp_input_method_v2 object as active, and makes any existing\nzwp_input_popup_surface_v2 objects visible.\n\nThe surrounding_text, and content_type events must follow before the\nnext done event if the text input supports the respective\nfunctionality.\n\nState set with this event is double-buffered. It will get applied on\nthe next zwp_input_method_v2.done event, and stay valid until changed."]
        pub fn activate(&self) -> () {
            let msg = Event::Activate;
            self.0.send(msg);
        }
        #[doc = "deactivate event\n\nNotification that no focused text input currently needs an active\ninput method on this seat.\n\nThis event marks the zwp_input_method_v2 object as inactive. The\ncompositor must make all existing zwp_input_popup_surface_v2 objects\ninvisible until the next activate event.\n\nState set with this event is double-buffered. It will get applied on\nthe next zwp_input_method_v2.done event, and stay valid until changed."]
        pub fn deactivate(&self) -> () {
            let msg = Event::Deactivate;
            self.0.send(msg);
        }
        #[doc = "surrounding text event\n\nUpdates the surrounding plain text around the cursor, excluding the\npreedit text.\n\nIf any preedit text is present, it is replaced with the cursor for the\npurpose of this event.\n\nThe argument text is a buffer containing the preedit string, and must\ninclude the cursor position, and the complete selection. It should\ncontain additional characters before and after these. There is a\nmaximum length of wayland messages, so text can not be longer than 4000\nbytes.\n\ncursor is the byte offset of the cursor within the text buffer.\n\nanchor is the byte offset of the selection anchor within the text\nbuffer. If there is no selected text, anchor must be the same as\ncursor.\n\nIf this event does not arrive before the first done event, the input\nmethod may assume that the text input does not support this\nfunctionality and ignore following surrounding_text events.\n\nValues set with this event are double-buffered. They will get applied\nand set to initial values on the next zwp_input_method_v2.done\nevent.\n\nThe initial state for affected fields is empty, meaning that the text\ninput does not support sending surrounding text. If the empty values\nget applied, subsequent attempts to change them may have no effect."]
        pub fn surrounding_text(&self, text: String, cursor: u32, anchor: u32) -> () {
            let msg = Event::SurroundingText {
                text: text,
                cursor: cursor,
                anchor: anchor,
            };
            self.0.send(msg);
        }
        #[doc = "indicates the cause of surrounding text change\n\nTells the input method why the text surrounding the cursor changed.\n\nWhenever the client detects an external change in text, cursor, or\nanchor position, it must issue this request to the compositor. This\nrequest is intended to give the input method a chance to update the\npreedit text in an appropriate way, e.g. by removing it when the user\nstarts typing with a keyboard.\n\ncause describes the source of the change.\n\nThe value set with this event is double-buffered. It will get applied\nand set to its initial value on the next zwp_input_method_v2.done\nevent.\n\nThe initial value of cause is input_method."]
        pub fn text_change_cause(&self, cause: super::zwp_text_input_v3::ChangeCause) -> () {
            let msg = Event::TextChangeCause { cause: cause };
            self.0.send(msg);
        }
        #[doc = "content purpose and hint\n\nIndicates the content type and hint for the current\nzwp_input_method_v2 instance.\n\nValues set with this event are double-buffered. They will get applied\non the next zwp_input_method_v2.done event.\n\nThe initial value for hint is none, and the initial value for purpose\nis normal."]
        pub fn content_type(
            &self,
            hint: super::zwp_text_input_v3::ContentHint,
            purpose: super::zwp_text_input_v3::ContentPurpose,
        ) -> () {
            let msg = Event::ContentType {
                hint: hint,
                purpose: purpose,
            };
            self.0.send(msg);
        }
        #[doc = "apply state\n\nAtomically applies state changes recently sent to the client.\n\nThe done event establishes and updates the state of the client, and\nmust be issued after any changes to apply them.\n\nText input state (content purpose, content hint, surrounding text, and\nchange cause) is conceptually double-buffered within an input method\ncontext.\n\nEvents modify the pending state, as opposed to the current state in use\nby the input method. A done event atomically applies all pending state,\nreplacing the current state. After done, the new pending state is as\ndocumented for each related request.\n\nEvents must be applied in the order of arrival.\n\nNeither current nor pending state are modified unless noted otherwise."]
        pub fn done(&self) -> () {
            let msg = Event::Done;
            self.0.send(msg);
        }
        #[doc = "input method unavailable\n\nThe input method ceased to be available.\n\nThe compositor must issue this event as the only event on the object if\nthere was another input_method object associated with the same seat at\nthe time of its creation.\n\nThe compositor must issue this request when the object is no longer\nuseable, e.g. due to seat removal.\n\nThe input method context becomes inert and should be destroyed after\ndeactivation is handled. Any further requests and events except for the\ndestroy request must be ignored."]
        pub fn unavailable(&self) -> () {
            let msg = Event::Unavailable;
            self.0.send(msg);
        }
    }
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_COMMIT_STRING_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_SET_PREEDIT_STRING_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DELETE_SURROUNDING_TEXT_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_COMMIT_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_GET_INPUT_POPUP_SURFACE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_GRAB_KEYBOARD_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_ACTIVATE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_DEACTIVATE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_SURROUNDING_TEXT_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_TEXT_CHANGE_CAUSE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_CONTENT_TYPE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_DONE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_UNAVAILABLE_SINCE: u32 = 1u32;
    static mut zwp_input_method_v2_requests_get_input_popup_surface_types: [*const wl_interface;
        2] = [
        unsafe {
            &super::zwp_input_popup_surface_v2::zwp_input_popup_surface_v2_interface
                as *const wl_interface
        },
        unsafe { &super::wl_surface::wl_surface_interface as *const wl_interface },
    ];
    static mut zwp_input_method_v2_requests_grab_keyboard_types: [*const wl_interface; 1] =
        [unsafe {
            &super::zwp_input_method_keyboard_grab_v2::zwp_input_method_keyboard_grab_v2_interface
                as *const wl_interface
        }];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_input_method_v2_requests: [wl_message; 7] = [
        wl_message {
            name: b"commit_string\0" as *const u8 as *const c_char,
            signature: b"s\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"set_preedit_string\0" as *const u8 as *const c_char,
            signature: b"sii\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"delete_surrounding_text\0" as *const u8 as *const c_char,
            signature: b"uu\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"commit\0" as *const u8 as *const c_char,
            signature: b"u\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"get_input_popup_surface\0" as *const u8 as *const c_char,
            signature: b"no\0" as *const u8 as *const c_char,
            types: unsafe {
                &zwp_input_method_v2_requests_get_input_popup_surface_types as *const _
            },
        },
        wl_message {
            name: b"grab_keyboard\0" as *const u8 as *const c_char,
            signature: b"n\0" as *const u8 as *const c_char,
            types: unsafe { &zwp_input_method_v2_requests_grab_keyboard_types as *const _ },
        },
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_input_method_v2_events: [wl_message; 7] = [
        wl_message {
            name: b"activate\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"deactivate\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"surrounding_text\0" as *const u8 as *const c_char,
            signature: b"suu\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"text_change_cause\0" as *const u8 as *const c_char,
            signature: b"u\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"content_type\0" as *const u8 as *const c_char,
            signature: b"uu\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"done\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"unavailable\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwp_input_method_v2_interface: wl_interface = wl_interface {
        name: b"zwp_input_method_v2\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 7,
        requests: unsafe { &zwp_input_method_v2_requests as *const _ },
        event_count: 7,
        events: unsafe { &zwp_input_method_v2_events as *const _ },
    };
}
#[doc = "popup surface\n\nThis interface marks a surface as a popup for interacting with an input\nmethod.\n\nThe compositor should place it near the active text input area. It must\nbe visible if and only if the input method is in the active state.\n\nThe client must not destroy the underlying wl_surface while the\nzwp_input_popup_surface_v2 object exists."]
pub mod zwp_input_popup_surface_v2 {
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
        #[doc = "set text input area position\n\nNotify about the position of the area of the text input expressed as a\nrectangle in surface local coordinates.\n\nThis is a hint to the input method telling it the relative position of\nthe text being entered."]
        TextInputRectangle {
            x: i32,
            y: i32,
            width: i32,
            height: i32,
        },
    }
    impl super::MessageGroup for Event {
        const MESSAGES: &'static [super::MessageDesc] = &[super::MessageDesc {
            name: "text_input_rectangle",
            since: 1,
            signature: &[
                super::ArgumentType::Int,
                super::ArgumentType::Int,
                super::ArgumentType::Int,
                super::ArgumentType::Int,
            ],
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
                Event::TextInputRectangle { .. } => 0,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Event::TextInputRectangle { .. } => 1,
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
                Event::TextInputRectangle {
                    x,
                    y,
                    width,
                    height,
                } => Message {
                    sender_id: sender_id,
                    opcode: 0,
                    args: smallvec![
                        Argument::Int(x),
                        Argument::Int(y),
                        Argument::Int(width),
                        Argument::Int(height),
                    ],
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
                Event::TextInputRectangle {
                    x,
                    y,
                    width,
                    height,
                } => {
                    let mut _args_array: [wl_argument; 4] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].i = x;
                    _args_array[1].i = y;
                    _args_array[2].i = width;
                    _args_array[3].i = height;
                    f(0, &mut _args_array)
                }
            }
        }
    }
    #[derive(Clone, Eq, PartialEq)]
    pub struct ZwpInputPopupSurfaceV2(Resource<ZwpInputPopupSurfaceV2>);
    impl AsRef<Resource<ZwpInputPopupSurfaceV2>> for ZwpInputPopupSurfaceV2 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwpInputPopupSurfaceV2>> for ZwpInputPopupSurfaceV2 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwpInputPopupSurfaceV2(value)
        }
    }
    impl From<ZwpInputPopupSurfaceV2> for Resource<ZwpInputPopupSurfaceV2> {
        #[inline]
        fn from(value: ZwpInputPopupSurfaceV2) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwpInputPopupSurfaceV2 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwpInputPopupSurfaceV2 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwp_input_popup_surface_v2";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwp_input_popup_surface_v2_interface }
        }
    }
    impl ZwpInputPopupSurfaceV2 {
        #[doc = "set text input area position\n\nNotify about the position of the area of the text input expressed as a\nrectangle in surface local coordinates.\n\nThis is a hint to the input method telling it the relative position of\nthe text being entered."]
        pub fn text_input_rectangle(&self, x: i32, y: i32, width: i32, height: i32) -> () {
            let msg = Event::TextInputRectangle {
                x: x,
                y: y,
                width: width,
                height: height,
            };
            self.0.send(msg);
        }
    }
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_TEXT_INPUT_RECTANGLE_SINCE: u32 = 1u32;
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_input_popup_surface_v2_requests: [wl_message; 1] = [wl_message {
        name: b"destroy\0" as *const u8 as *const c_char,
        signature: b"\0" as *const u8 as *const c_char,
        types: unsafe { &types_null as *const _ },
    }];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_input_popup_surface_v2_events: [wl_message; 1] = [wl_message {
        name: b"text_input_rectangle\0" as *const u8 as *const c_char,
        signature: b"iiii\0" as *const u8 as *const c_char,
        types: unsafe { &types_null as *const _ },
    }];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwp_input_popup_surface_v2_interface: wl_interface = wl_interface {
        name: b"zwp_input_popup_surface_v2\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 1,
        requests: unsafe { &zwp_input_popup_surface_v2_requests as *const _ },
        event_count: 1,
        events: unsafe { &zwp_input_popup_surface_v2_events as *const _ },
    };
}
#[doc = "keyboard grab\n\nThe zwp_input_method_keyboard_grab_v2 interface represents an exclusive\ngrab of the wl_keyboard interface associated with the seat."]
pub mod zwp_input_method_keyboard_grab_v2 {
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
        #[doc = "release the grab object\n\n\n\nThis is a destructor, once received this object cannot be used any longer."]
        Release,
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[super::MessageDesc {
            name: "release",
            since: 1,
            signature: &[],
            destructor: true,
        }];
        type Map = super::ResourceMap;
        fn is_destructor(&self) -> bool {
            match *self {
                Request::Release => true,
            }
        }
        fn opcode(&self) -> u16 {
            match *self {
                Request::Release => 0,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::Release => 1,
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
        #[doc = "keyboard mapping\n\nThis event provides a file descriptor to the client which can be\nmemory-mapped to provide a keyboard mapping description."]
        Keymap {
            format: super::wl_keyboard::KeymapFormat,
            fd: ::std::os::unix::io::RawFd,
            size: u32,
        },
        #[doc = "key event\n\nA key was pressed or released.\nThe time argument is a timestamp with millisecond granularity, with an\nundefined base."]
        Key {
            serial: u32,
            time: u32,
            key: u32,
            state: super::wl_keyboard::KeyState,
        },
        #[doc = "modifier and group state\n\nNotifies clients that the modifier and/or group state has changed, and\nit should update its local state."]
        Modifiers {
            serial: u32,
            mods_depressed: u32,
            mods_latched: u32,
            mods_locked: u32,
            group: u32,
        },
        #[doc = "repeat rate and delay\n\nInforms the client about the keyboard's repeat rate and delay.\n\nThis event is sent as soon as the zwp_input_method_keyboard_grab_v2\nobject has been created, and is guaranteed to be received by the\nclient before any key press event.\n\nNegative values for either rate or delay are illegal. A rate of zero\nwill disable any repeating (regardless of the value of delay).\n\nThis event can be sent later on as well with a new value if necessary,\nso clients should continue listening for the event past the creation\nof zwp_input_method_keyboard_grab_v2."]
        RepeatInfo { rate: i32, delay: i32 },
    }
    impl super::MessageGroup for Event {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "keymap",
                since: 1,
                signature: &[
                    super::ArgumentType::Uint,
                    super::ArgumentType::Fd,
                    super::ArgumentType::Uint,
                ],
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
                name: "repeat_info",
                since: 1,
                signature: &[super::ArgumentType::Int, super::ArgumentType::Int],
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
                Event::Keymap { .. } => 0,
                Event::Key { .. } => 1,
                Event::Modifiers { .. } => 2,
                Event::RepeatInfo { .. } => 3,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Event::Keymap { .. } => 1,
                Event::Key { .. } => 1,
                Event::Modifiers { .. } => 1,
                Event::RepeatInfo { .. } => 1,
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
                Event::Keymap { format, fd, size } => Message {
                    sender_id: sender_id,
                    opcode: 0,
                    args: smallvec![
                        Argument::Uint(format.to_raw()),
                        Argument::Fd(fd),
                        Argument::Uint(size),
                    ],
                },
                Event::Key {
                    serial,
                    time,
                    key,
                    state,
                } => Message {
                    sender_id: sender_id,
                    opcode: 1,
                    args: smallvec![
                        Argument::Uint(serial),
                        Argument::Uint(time),
                        Argument::Uint(key),
                        Argument::Uint(state.to_raw()),
                    ],
                },
                Event::Modifiers {
                    serial,
                    mods_depressed,
                    mods_latched,
                    mods_locked,
                    group,
                } => Message {
                    sender_id: sender_id,
                    opcode: 2,
                    args: smallvec![
                        Argument::Uint(serial),
                        Argument::Uint(mods_depressed),
                        Argument::Uint(mods_latched),
                        Argument::Uint(mods_locked),
                        Argument::Uint(group),
                    ],
                },
                Event::RepeatInfo { rate, delay } => Message {
                    sender_id: sender_id,
                    opcode: 3,
                    args: smallvec![Argument::Int(rate), Argument::Int(delay),],
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
                Event::Keymap { format, fd, size } => {
                    let mut _args_array: [wl_argument; 3] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = format.to_raw();
                    _args_array[1].h = fd;
                    _args_array[2].u = size;
                    f(0, &mut _args_array)
                }
                Event::Key {
                    serial,
                    time,
                    key,
                    state,
                } => {
                    let mut _args_array: [wl_argument; 4] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = serial;
                    _args_array[1].u = time;
                    _args_array[2].u = key;
                    _args_array[3].u = state.to_raw();
                    f(1, &mut _args_array)
                }
                Event::Modifiers {
                    serial,
                    mods_depressed,
                    mods_latched,
                    mods_locked,
                    group,
                } => {
                    let mut _args_array: [wl_argument; 5] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = serial;
                    _args_array[1].u = mods_depressed;
                    _args_array[2].u = mods_latched;
                    _args_array[3].u = mods_locked;
                    _args_array[4].u = group;
                    f(2, &mut _args_array)
                }
                Event::RepeatInfo { rate, delay } => {
                    let mut _args_array: [wl_argument; 2] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].i = rate;
                    _args_array[1].i = delay;
                    f(3, &mut _args_array)
                }
            }
        }
    }
    #[derive(Clone, Eq, PartialEq)]
    pub struct ZwpInputMethodKeyboardGrabV2(Resource<ZwpInputMethodKeyboardGrabV2>);
    impl AsRef<Resource<ZwpInputMethodKeyboardGrabV2>> for ZwpInputMethodKeyboardGrabV2 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwpInputMethodKeyboardGrabV2>> for ZwpInputMethodKeyboardGrabV2 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwpInputMethodKeyboardGrabV2(value)
        }
    }
    impl From<ZwpInputMethodKeyboardGrabV2> for Resource<ZwpInputMethodKeyboardGrabV2> {
        #[inline]
        fn from(value: ZwpInputMethodKeyboardGrabV2) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwpInputMethodKeyboardGrabV2 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwpInputMethodKeyboardGrabV2 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwp_input_method_keyboard_grab_v2";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwp_input_method_keyboard_grab_v2_interface }
        }
    }
    impl ZwpInputMethodKeyboardGrabV2 {
        #[doc = "keyboard mapping\n\nThis event provides a file descriptor to the client which can be\nmemory-mapped to provide a keyboard mapping description."]
        pub fn keymap(
            &self,
            format: super::wl_keyboard::KeymapFormat,
            fd: ::std::os::unix::io::RawFd,
            size: u32,
        ) -> () {
            let msg = Event::Keymap {
                format: format,
                fd: fd,
                size: size,
            };
            self.0.send(msg);
        }
        #[doc = "key event\n\nA key was pressed or released.\nThe time argument is a timestamp with millisecond granularity, with an\nundefined base."]
        pub fn key(
            &self,
            serial: u32,
            time: u32,
            key: u32,
            state: super::wl_keyboard::KeyState,
        ) -> () {
            let msg = Event::Key {
                serial: serial,
                time: time,
                key: key,
                state: state,
            };
            self.0.send(msg);
        }
        #[doc = "modifier and group state\n\nNotifies clients that the modifier and/or group state has changed, and\nit should update its local state."]
        pub fn modifiers(
            &self,
            serial: u32,
            mods_depressed: u32,
            mods_latched: u32,
            mods_locked: u32,
            group: u32,
        ) -> () {
            let msg = Event::Modifiers {
                serial: serial,
                mods_depressed: mods_depressed,
                mods_latched: mods_latched,
                mods_locked: mods_locked,
                group: group,
            };
            self.0.send(msg);
        }
        #[doc = "repeat rate and delay\n\nInforms the client about the keyboard's repeat rate and delay.\n\nThis event is sent as soon as the zwp_input_method_keyboard_grab_v2\nobject has been created, and is guaranteed to be received by the\nclient before any key press event.\n\nNegative values for either rate or delay are illegal. A rate of zero\nwill disable any repeating (regardless of the value of delay).\n\nThis event can be sent later on as well with a new value if necessary,\nso clients should continue listening for the event past the creation\nof zwp_input_method_keyboard_grab_v2."]
        pub fn repeat_info(&self, rate: i32, delay: i32) -> () {
            let msg = Event::RepeatInfo {
                rate: rate,
                delay: delay,
            };
            self.0.send(msg);
        }
    }
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_RELEASE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_KEYMAP_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_KEY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_MODIFIERS_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_REPEAT_INFO_SINCE: u32 = 1u32;
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_input_method_keyboard_grab_v2_requests: [wl_message; 1] = [wl_message {
        name: b"release\0" as *const u8 as *const c_char,
        signature: b"\0" as *const u8 as *const c_char,
        types: unsafe { &types_null as *const _ },
    }];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_input_method_keyboard_grab_v2_events: [wl_message; 4] = [
        wl_message {
            name: b"keymap\0" as *const u8 as *const c_char,
            signature: b"uhu\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
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
            name: b"repeat_info\0" as *const u8 as *const c_char,
            signature: b"ii\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwp_input_method_keyboard_grab_v2_interface: wl_interface = wl_interface {
        name: b"zwp_input_method_keyboard_grab_v2\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 1,
        requests: unsafe { &zwp_input_method_keyboard_grab_v2_requests as *const _ },
        event_count: 4,
        events: unsafe { &zwp_input_method_keyboard_grab_v2_events as *const _ },
    };
}
#[doc = "input method manager\n\nThe input method manager allows the client to become the input method on\na chosen seat.\n\nNo more than one input method must be associated with any seat at any\ngiven time."]
pub mod zwp_input_method_manager_v2 {
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
        #[doc = "request an input method object\n\nRequest a new input zwp_input_method_v2 object associated with a given\nseat."]
        GetInputMethod {
            seat: super::wl_seat::WlSeat,
            input_method: Main<super::zwp_input_method_v2::ZwpInputMethodV2>,
        },
        #[doc = "destroy the input method manager\n\nDestroys the zwp_input_method_manager_v2 object.\n\nThe zwp_input_method_v2 objects originating from it remain valid.\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "get_input_method",
                since: 1,
                signature: &[super::ArgumentType::Object, super::ArgumentType::NewId],
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
                Request::GetInputMethod { .. } => 0,
                Request::Destroy => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::GetInputMethod { .. } => 1,
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
                    super::zwp_input_method_v2::ZwpInputMethodV2,
                >(version, meta.child())),
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            match msg.opcode {
                0 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::GetInputMethod {
                        seat: {
                            if let Some(Argument::Object(val)) = args.next() {
                                map.get(val).ok_or(())?.into()
                            } else {
                                return Err(());
                            }
                        },
                        input_method: {
                            if let Some(Argument::NewId(val)) = args.next() {
                                map.get_new(val).ok_or(())?
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
                    Ok(Request::GetInputMethod {
                        seat: Resource::<super::wl_seat::WlSeat>::from_c_ptr(_args[0].o as *mut _)
                            .into(),
                        input_method: {
                            let me = Resource::<ZwpInputMethodManagerV2>::from_c_ptr(obj as *mut _);
                            me.make_child_for::<super::zwp_input_method_v2::ZwpInputMethodV2>(
                                _args[1].n,
                            )
                            .unwrap()
                        },
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
    pub struct ZwpInputMethodManagerV2(Resource<ZwpInputMethodManagerV2>);
    impl AsRef<Resource<ZwpInputMethodManagerV2>> for ZwpInputMethodManagerV2 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwpInputMethodManagerV2>> for ZwpInputMethodManagerV2 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwpInputMethodManagerV2(value)
        }
    }
    impl From<ZwpInputMethodManagerV2> for Resource<ZwpInputMethodManagerV2> {
        #[inline]
        fn from(value: ZwpInputMethodManagerV2) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwpInputMethodManagerV2 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwpInputMethodManagerV2 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwp_input_method_manager_v2";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwp_input_method_manager_v2_interface }
        }
    }
    impl ZwpInputMethodManagerV2 {}
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_GET_INPUT_METHOD_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    static mut zwp_input_method_manager_v2_requests_get_input_method_types: [*const wl_interface;
        2] = [
        unsafe { &super::wl_seat::wl_seat_interface as *const wl_interface },
        unsafe {
            &super::zwp_input_method_v2::zwp_input_method_v2_interface as *const wl_interface
        },
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_input_method_manager_v2_requests: [wl_message; 2] = [
        wl_message {
            name: b"get_input_method\0" as *const u8 as *const c_char,
            signature: b"on\0" as *const u8 as *const c_char,
            types: unsafe {
                &zwp_input_method_manager_v2_requests_get_input_method_types as *const _
            },
        },
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwp_input_method_manager_v2_interface: wl_interface = wl_interface {
        name: b"zwp_input_method_manager_v2\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 2,
        requests: unsafe { &zwp_input_method_manager_v2_requests as *const _ },
        event_count: 0,
        events: NULLPTR as *const wl_message,
    };
}
