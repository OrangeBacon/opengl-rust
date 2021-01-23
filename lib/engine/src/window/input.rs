use std::collections::HashMap;

use super::scancode::Scancode;

/// Current state of a key
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum KeyState {
    /// The key is not currently pressed
    None,

    /// The key was pressed down on this frame
    Down,

    /// The key is being held down
    Hold,

    /// The key was released on this frame
    Up,
}

/// The current frame's mouse, scroll wheel and mouse button state
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct MouseState {
    /// Current mouse horizontal position
    x: i32,

    /// Current mouse vertical position
    y: i32,

    /// How much the mouse moved horizontally this frame
    delta_x: i32,

    /// How much the mouse moved vertically this frame
    delta_y: i32,

    /// The current mouse wheel horizontal location
    wheel_x: i32,

    /// The current mouse wheel vertical location
    wheel_y: i32,

    /// How much the mouse wheel moved horizontally this frame
    wheel_delta_x: i32,

    /// How much the mouse wheel moved vertically this frame
    wheel_delta_y: i32,

    /// The state of the left click button
    left_button: bool,

    /// The state of the middle mouse button
    middle_button: bool,

    /// The state of the right mouse button
    right_button: bool,

    /// The state of the fourth mouse button (back)
    mouse_four: bool,

    /// The state of the fifth mouse button (forward)
    mouse_five: bool,
}

/// The state of the user input on the current frame
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct InputState {
    /// The state of the mouse
    mouse: MouseState,

    /// The state of the keyboard
    keys: HashMap<Scancode, KeyState>,
}

impl InputState {
    /// updates the state for a new frame
    pub fn update(&mut self, mouse_state: MouseState) {
        self.mouse = mouse_state;

        // update the mouse buttons depending upon what they were like last frame
        self.keys = self
            .keys
            .iter()
            .map(|(scan, state)| {
                let new_state = match state {
                    // Keys that were pressed down last frame are now held down
                    KeyState::Down => KeyState::Hold,

                    // Keys that were released last frame are now not pressed
                    KeyState::Up => KeyState::None,

                    // Other keys do not change
                    a => *a,
                };
                (*scan, new_state)
            })
            .collect();
    }

    /// Get the current state of a key
    pub fn key_state(&self, key: Scancode) -> KeyState {
        *self.keys.get(&key).unwrap_or(&KeyState::None)
    }

    pub(crate) fn set_key_state(&mut self, key: Scancode, state: KeyState) {
        self.keys.insert(key, state);
    }

    /// Is a key currently pressed down
    pub fn is_key_pressed(&self, key: Scancode) -> bool {
        let key = *self.keys.get(&key).unwrap_or(&KeyState::None);
        if key == KeyState::Down || key == KeyState::Hold {
            true
        } else {
            false
        }
    }

    /// Get the current (x, y) position of the mouse
    pub fn mouse_position(&self) -> (i32, i32) {
        (self.mouse.x, self.mouse.y)
    }

    /// Set the current mouse position this frame
    pub(crate) fn set_mouse_position(&mut self, x: i32, y: i32) {
        self.mouse.x = x;
        self.mouse.y = y;
    }

    /// Get the amount the mouse moved in the current frame
    pub fn mouse_delta(&self) -> (i32, i32) {
        (self.mouse.delta_x, self.mouse.delta_y)
    }

    /// Set the amount the mouse moved in the current frame
    pub(crate) fn set_mouse_delta(&mut self, x: i32, y: i32) {
        self.mouse.delta_x = x;
        self.mouse.delta_y = y;
    }

    /// Get the cumulative scroll position of the mouse wheel
    /// result = (horizontal, vertical), horizontal scroll might not be possible
    /// depending upon the mouse
    pub fn wheel_position(&self) -> (i32, i32) {
        (self.mouse.wheel_x, self.mouse.wheel_y)
    }

    /// Set the current mouse scroll position
    pub(crate) fn set_wheel_position(&mut self, x: i32, y: i32) {
        self.mouse.wheel_x = x;
        self.mouse.wheel_y = y;
    }

    /// Get how much the mouse wheel has moved in the current frame
    /// result = (horizontal, vertical)
    pub fn wheel_delta(&self) -> (i32, i32) {
        (self.mouse.wheel_delta_x, self.mouse.wheel_delta_y)
    }

    /// Set the current frames's scroll wheel change.  This does not
    /// update the scroll wheel position, just the delta.
    pub(crate) fn set_wheel_delta(&mut self, x: i32, y: i32) {
        self.mouse.wheel_delta_x = x;
        self.mouse.wheel_delta_y = y;
    }

    /// Is the left mouse button pressed
    pub fn mouse_left(&self) -> bool {
        self.mouse.left_button
    }

    /// Set if the left mouse button is pressed
    pub(crate) fn set_mouse_left(&mut self, value: bool) {
        self.mouse.left_button = value;
    }

    /// Is the middle mouse button pressed
    pub fn mouse_middle(&self) -> bool {
        self.mouse.middle_button
    }

    /// Set if the middle mouse button is pressed
    pub(crate) fn set_mouse_middle(&mut self, value: bool) {
        self.mouse.middle_button = value;
    }

    /// Is the right mouse button pressed
    pub fn mouse_right(&self) -> bool {
        self.mouse.right_button
    }

    /// Set if the right mouse button is pressed
    pub(crate) fn set_mouse_right(&mut self, value: bool) {
        self.mouse.right_button = value;
    }

    /// Is the fourth mouse button pressed
    pub fn mouse_four(&self) -> bool {
        self.mouse.mouse_four
    }

    /// Set if the fourth mouse button is pressed
    pub(crate) fn set_mouse_four(&mut self, value: bool) {
        self.mouse.mouse_four = value;
    }

    /// Is the fifth mouse button pressed
    pub fn mouse_five(&self) -> bool {
        self.mouse.mouse_five
    }

    /// Set if the fifth mouse button pressed
    pub(crate) fn set_mouse_five(&mut self, value: bool) {
        self.mouse.mouse_five = value;
    }
}
