/// A single button on the keyboard.
/// Not useful for text input, use the text input event for that.
/// See https://wiki.libsdl.org/SDL_Keycode
/// See https://usb.org/sites/default/files/hut1_21.pdf
#[derive(Copy, Clone, Eq, PartialEq, Hash, PartialOrd, Ord, Debug)]
pub enum Scancode {
    // Keyboard Letter keys
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,

    // Numeric keys above the main keys
    Zero,
    One,
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,

    // Keypad numeric keys
    KpZero,
    KpOne,
    KpTwo,
    KpThree,
    KpFour,
    KpFive,
    KpSix,
    KpSeven,
    KpEight,
    KpNine,
    KpZeroZero,
    KpZeroZeroZero,

    // Keypad hexadecimal number keys
    KpA,
    KpB,
    KpC,
    KpD,
    KpE,
    KpF,

    // Keypad symbols
    KpAnd,
    KpAndAnd,
    KpAt,
    KpCaret,
    KpColon,
    KpComma,
    KpDash,
    KpEquals,
    KpEqualsAs400,
    KpExclamation,
    KpHash,
    KpLeftBrace,
    KpLeftParen,
    KpLess,
    KpPercent,
    KpPeriod,
    KpPipe,
    KpPipePipe,
    KpPlusMinus,
    KpRightBrace,
    KpRightParen,
    KpSlash,
    KpSpace,
    KpStar,
    KpTab,
    KpXor,
    KpPlus,
    KpGreater,

    // Keypad control characters
    KpBackspace,
    KpClear,
    KpClearEntry,
    KpEnter,
    KpMemAdd,
    KpMemClear,
    KpMemDivide,
    KpMemMultiply,
    KpMemRecall,
    KpMemSubtract,
    KpMemStore,
    KpBinary,
    KpOctal,
    KpDecimal,
    KpHex,
    KpPower,

    // Symbol Keys
    Apostrophe,
    Backslash,
    Comma,
    Equals,
    Grave,
    LeftSquareBracket,
    LeftBracket,
    Dash,
    Period,
    RightSquareBracket,
    RightBracket,
    SemiColon,
    Slash,
    Space,
    Tab,
    AltHash,
    AltBackslash,

    // Arrow keys
    Down,
    Left,
    Right,
    Up,

    // Control keys
    Backspace,
    CapsLock,
    Copy,
    Cut,
    Delete,
    End,
    Escape,
    Home,
    Insert,
    LeftAlt,
    LeftControl,
    LeftShift,
    LeftMeta,
    Menu,
    NumLock,
    PageDown,
    PageUp,
    Paste,
    PrintScreen,
    RightAlt,
    RightControl,
    Return,
    Return2,
    RightMeta,
    RightShift,
    ScrollLock,
    Undo,

    // Function keys
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,

    // International keys
    InternationalOne,
    InternationalTwo,
    InternationalThree,
    InternationalFour,
    InternationalFive,
    InternationalSix,
    InternationalSeven,
    InternationalEight,
    InternationalNine,

    // Language keys
    LangOne,
    LangTwo,
    LangThree,
    LangFour,
    LangFive,
    LangSix,
    LangSeven,
    LangEight,
    LangNine,

    // Audio keys
    AudioMute,
    AudioNext,
    AudioPlay,
    AudioPrev,
    AudioStop,
    Mute,
    VolumeDown,
    VolumeUp,

    // Application keys
    Application,
    App1,
    App2,
    AppBack,
    AppBookmark,
    AppForward,
    AppHome,
    AppRefresh,
    AppSearch,
    AppStop,

    // Brightness keys
    BrightnessDown,
    BrightnessUp,

    // Keyboard illumination keys
    KeyboardIllumDown,
    KeyboardIllumUp,
    KeyboardIllumToggle,

    // Miscellaneous keys
    Again,
    AltErase,
    Calculator,
    Cancel,
    Clear,
    ClearAgain,
    Computer,
    CrSel,
    CurrencySubUnit,
    CurrencyUnit,
    DecimalSeparator,
    DisplaySwitch,
    Eject,
    ExSel,
    Find,
    Help,
    Mail,
    MediaSelect,
    Mode,
    ModeSwitch,
    Oper,
    Out,
    Power,
    Prior,
    Select,
    Separator,
    Sleep,
    Stop,
    SysReq,
    ThousandsSeparator,
    WorldWideWeb,
    Unknown,
    Pause,
    Execute,
}