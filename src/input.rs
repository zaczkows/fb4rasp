#[allow(dead_code)]
#[allow(non_camel_case_types)]
#[derive(Debug)]
pub(crate) enum Type {
    EV_SYN = 0x00,
    EV_KEY = 0x01,
    EV_REL = 0x02,
    EV_ABS = 0x03,
    EV_MSC = 0x04,
    EV_SW = 0x05,
    EV_LED = 0x11,
    EV_SND = 0x12,
    EV_REP = 0x14,
    EV_FF = 0x15,
    EV_PWR = 0x16,
    EV_FF_STATUS = 0x17,
}

impl num_traits::cast::FromPrimitive for Type {
    fn from_i64(n: i64) -> Option<Self> {
        Some(unsafe { std::mem::transmute::<u8, Self>(n as u8) })
    }

    fn from_u64(n: u64) -> Option<Self> {
        Some(unsafe { std::mem::transmute::<u8, Self>(n as u8) })
    }
}

/*
 * Relative axes
 */
#[allow(dead_code)]
#[allow(non_camel_case_types)]
#[derive(Debug)]
pub(crate) enum Rel {
    REL_X = 0x00,
    REL_Y = 0x01,
    REL_Z = 0x02,
    REL_RX = 0x03,
    REL_RY = 0x04,
    REL_RZ = 0x05,
    REL_HWHEEL = 0x06,
    REL_DIAL = 0x07,
    REL_WHEEL = 0x08,
    REL_MISC = 0x09,
    REL_MAX = 0x0f,
}

impl num_traits::cast::FromPrimitive for Rel {
    fn from_i64(n: i64) -> Option<Self> {
        Some(unsafe { std::mem::transmute::<u8, Self>(n as u8) })
    }

    fn from_u64(n: u64) -> Option<Self> {
        Some(unsafe { std::mem::transmute::<u8, Self>(n as u8) })
    }
}

/*
 * Absolute axes
 */
#[allow(dead_code)]
#[allow(non_camel_case_types)]
#[derive(Debug)]
pub(crate) enum Abs {
    ABS_X = 0x00,
    ABS_Y = 0x01,
    ABS_Z = 0x02,
    ABS_RX = 0x03,
    ABS_RY = 0x04,
    ABS_RZ = 0x05,
    ABS_THROTTLE = 0x06,
    ABS_RUDDER = 0x07,
    ABS_WHEEL = 0x08,
    ABS_GAS = 0x09,
    ABS_BRAKE = 0x0a,
    ABS_HAT0X = 0x10,
    ABS_HAT0Y = 0x11,
    ABS_HAT1X = 0x12,
    ABS_HAT1Y = 0x13,
    ABS_HAT2X = 0x14,
    ABS_HAT2Y = 0x15,
    ABS_HAT3X = 0x16,
    ABS_HAT3Y = 0x17,
    ABS_PRESSURE = 0x18,
    ABS_DISTANCE = 0x19,
    ABS_TILT_X = 0x1a,
    ABS_TILT_Y = 0x1b,
    ABS_TOOL_WIDTH = 0x1c,
    ABS_VOLUME = 0x20,
    ABS_MISC = 0x28,
    ABS_MT_SLOT = 0x2f,        /* MT slot being modified */
    ABS_MT_TOUCH_MAJOR = 0x30, /* Major axis of touching ellipse */
    ABS_MT_TOUCH_MINOR = 0x31, /* Minor axis (omit if circular) */
    ABS_MT_WIDTH_MAJOR = 0x32, /* Major axis of approaching ellipse */
    ABS_MT_WIDTH_MINOR = 0x33, /* Minor axis (omit if circular) */
    ABS_MT_ORIENTATION = 0x34, /* Ellipse orientation */
    ABS_MT_POSITION_X = 0x35,  /* Center X touch position */
    ABS_MT_POSITION_Y = 0x36,  /* Center Y touch position */
    ABS_MT_TOOL_TYPE = 0x37,   /* Type of touching device */
    ABS_MT_BLOB_ID = 0x38,     /* Group a set of packets as a blob */
    ABS_MT_TRACKING_ID = 0x39, /* Unique ID of initiated contact */
    ABS_MT_PRESSURE = 0x3a,    /* Pressure on contact area */
    ABS_MT_DISTANCE = 0x3b,    /* Contact hover distance */
    ABS_MT_TOOL_X = 0x3c,      /* Center X tool position */
    ABS_MT_TOOL_Y = 0x3d,      /* Center Y tool position */
    ABS_MAX = 0x3f,
}

impl num_traits::cast::FromPrimitive for Abs {
    fn from_i64(n: i64) -> Option<Self> {
        Some(unsafe { std::mem::transmute::<u8, Self>(n as u8) })
    }

    fn from_u64(n: u64) -> Option<Self> {
        Some(unsafe { std::mem::transmute::<u8, Self>(n as u8) })
    }
}

#[derive(Debug)]
pub(crate) enum EvType {
    Unknown(Type),
    Relative(Rel),
    Absolute(Abs),
}

#[derive(Debug)]
pub(crate) struct Event {
    pub useconds: i64,
    pub r#type: EvType,
    pub value: i32,
}

impl From<evdev::raw::input_event> for Event {
    fn from(ie: evdev::raw::input_event) -> Self {
        use num_traits::cast::FromPrimitive;

        let t = Type::from_u16(ie._type).unwrap();
        let ev_type = match t {
            Type::EV_REL => EvType::Relative(Rel::from_u16(ie.code).unwrap()),
            Type::EV_ABS => EvType::Absolute(Abs::from_u16(ie.code).unwrap()),
            t => EvType::Unknown(t),
        };

        Event {
            useconds: ie.time.tv_sec as i64 * 1000000 + ie.time.tv_usec as i64,
            r#type: ev_type,
            value: ie.value,
        }
    }
}
