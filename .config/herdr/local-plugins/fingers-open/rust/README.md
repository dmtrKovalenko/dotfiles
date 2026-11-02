# Native Herdr Fingers

This crate builds the complete custom Fingers plugin as one executable. See the
[parent README](../README.md) for installation and verification.

The hint engine is vendored from [nathan-poncet/herdr-fingers](https://github.com/nathan-poncet/herdr-fingers)
at commit `b763a3ffe4e8f909584f2e19229f95719f3f4314`, under its included MIT license.
Local changes retain the overlay frame during actions, add bounded Herdr requests,
and route `:fingers-open:` directly to the native implementation in `src/custom`.
The binary embeds `../herdr_fingers.lua` for installation into Neovim.
