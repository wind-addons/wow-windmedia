-- Media registration loader
-- Version: __VERSION__

local ADDON_NAME, addon = ...
local data = addon and addon.data

if not data then
	return
end

local LSM = LibStub("LibSharedMedia-3.0", true)
if not LSM then
	return
end

local BASE_PATH = "Interface\\AddOns\\" .. ADDON_NAME .. "\\"

local LOCALE_FLAGS = {
	koKR = LSM.LOCALE_BIT_koKR,
	ruRU = LSM.LOCALE_BIT_ruRU,
	zhCN = LSM.LOCALE_BIT_zhCN,
	zhTW = LSM.LOCALE_BIT_zhTW,
	western = LSM.LOCALE_BIT_western,
}

local function calc_locale_mask(locales)
	local mask = 0
	for _, name in ipairs(locales) do
		local bit = LOCALE_FLAGS[name]
		if bit then
			mask = mask + bit
		end
	end
	return (mask > 0) and mask or LOCALE_FLAGS.western
end

local function register_entry(entry)
	local entry_type = entry.type
	local key = entry.key
	local file = BASE_PATH .. entry.file

	if entry_type == "statusbar" then
		LSM:Register("statusbar", key, file)
	elseif entry_type == "background" then
		LSM:Register("background", key, file)
	elseif entry_type == "border" then
		LSM:Register("border", key, file)
	elseif entry_type == "font" then
		local meta = entry.metadata
		local locales = meta and meta.locales
		local mask = (locales and #locales > 0) and calc_locale_mask(locales) or nil
		LSM:Register("font", key, file, mask)
	elseif entry_type == "sound" then
		LSM:Register("sound", key, file)
	end
end

for i = 1, #data.entries do
	register_entry(data.entries[i])
end
