use {
    crate::{att::AttUuid, uuid::Uuid16},
    bitflags::bitflags,
};

bitflags! {
    pub struct Properties: u8 {
        const BROADCAST    = 0x01;
        const READ         = 0x02;
        const WRITE_NO_RSP = 0x04;
        const WRITE        = 0x08;
        const NOTIFY       = 0x10;
        const INDICATE     = 0x20;
        const AUTH_WRITES  = 0x40;
        const EXTENDED     = 0x80;
    }
}

/// Bitwise or operation on `bitflags!` types that works in a `const` context.
macro_rules! const_or {
    (
        $($t:ident :: $bit:ident)|+
    ) => {{
        <const_or!(@[$($t)+])>::from_bits_truncate($(($t :: $bit).bits())|+)
    }};

    (
        @[$first:tt $($rest:tt)*]
    ) => { $first };
}

pub trait Characteristic {
    const PROPS: Properties;

    /// The UUID assigned to the characteristic type.
    const UUID: AttUuid;
}

pub struct BatteryLevel {
    /// Battery level in percent (0-100).
    percentage: u8,
}

impl BatteryLevel {
    pub fn new(percentage: u8) -> Self {
        assert!(percentage <= 100);
        Self { percentage }
    }

    pub fn percentage(&self) -> u8 {
        self.percentage
    }
}

impl Characteristic for BatteryLevel {
    const PROPS: Properties = const_or!(Properties::READ | Properties::WRITE);
    const UUID: AttUuid = AttUuid::Uuid16(Uuid16(0x2A19));
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Appearance {
    Unknown = 0,
    GenericPhone = 64,
    GenericComputer = 128,
    GenericWatch = 192,
    SportsWatch = 193,
    GenericClock = 256,
    GenericDisplay = 320,   // yeah good luck with that
    GenericRemoteControl = 384,
    GenericEyeGlasses = 448,
    GenericTag = 512,
    GenericKeyring = 576,
    GenericMediaPlayer = 640,
    GenericBarcodeScanner = 704,
    GenericThermometer = 768,
    ThermometerEar = 769,
    GenericHeartRateSensor = 832,
    HeartRateBelt = 833,

    GenericBloodPressure = 896,
    BloodPressureArm = 897,
    BloodPressureWrist = 898,

    HumanInterfaceDevice = 960,
    Keyboard = 961,
    Mouse = 962,
    Joystick = 963,
    Gamepad = 964,
    DigitizerTablet = 965,
    CardReader = 966,
    DigitalPen = 967,
    BarcodeScanner = 968,

    GenericGlucoseMeter = 1024,

    GenericRunningWalkingSensor = 1088,
    RunningWalkingSensorInShoe = 1089,
    RunningWalkingSensorOnShoe = 1090,
    RunningWalkingSensorOnHip = 1091,
    GenericCycling = 1152,
    CyclingComputer = 1153,
    CyclingSpeedSensor = 1154,
    CyclingCadenceSensor = 1155,
    CyclingPowerSensor = 1156,
    CyclingSpeedAndCadenceSensor = 1157,

    GenericPulseOximeter = 3136,
    PulseOximeterFingertip = 3137,
    PulseOximeterWristWorn = 3138,

    GenericWeightScale = 3200,

    GenericPersonalMobilityDevice = 3264,
    PoweredWheelchair = 3265,
    MobilityScooter = 3266,
    GenericContinuousGlucoseMonitor = 3328,

    GenericInsulinPump = 3392,  // no
    DurableInsulinPump = 3393,  // no
    PatchInsulingPump = 3396,   // no
    InsulinPen = 3400,  // no
    GenericMedicationDelivery = 3456,   // don't even think about it

    GenericOutdoorSportsActivity = 5184,
    LocationDisplayService = 5185,
    LocationAndNavigationDisplayService = 5186,
    LocationPod = 5187,
    LocationAndNavigationPod = 5188,
}
