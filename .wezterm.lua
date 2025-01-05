-- Pull in the wezterm API
local wezterm = require("wezterm")
local action = wezterm.action

-- This will hold the configuration.
local config = wezterm.config_builder()

-- This is where you actually apply your config choices

-- For example, changing the color scheme:
config.color_scheme = "Catppuccin Mocha"
config.automatically_reload_config = true
config.font_size = 21.0
config.font = wezterm.font("BerkeleyMono Nerd Font")
config.hide_tab_bar_if_only_one_tab = true
config.enable_kitty_keyboard = true
config.window_decorations = "RESIZE"
config.enable_tab_bar = false

config.window_background_image_hsb = {
	brightness = 1.0,
	hue = 1.0,
	saturation = 1.0,
}

config.keys = {
	{
		key = "L",
		mods = "CTRL",
		action = action.ClearScrollback("ScrollbackOnly"),
	},
}

-- Set the background mode to prevent stretching
config.background = {
	{
		source = {
			File = "/Users/dmtrkovalenko/Downloads/background.png",
		},
		-- This will maintain the aspect ratio and scale to fit
		repeat_x = "NoRepeat",
		repeat_y = "NoRepeat",
		vertical_align = "Middle",
		horizontal_align = "Center",
		-- You can adjust this to change how the image fits
		-- 'Cover' will ensure the image covers the entire window
		-- 'Contain' will ensure the entire image is visible
		hsb = config.window_background_image_hsb,
		-- Adjust opacity as needed (0.0 to 1.0)
		opacity = 1.0,
		-- Adjust this value to change the size of the image
		-- 1.0 means original size, less than 1.0 will shrink, greater will enlarge
	},
}

-- and finally, return the configuration to wezterm
return config
