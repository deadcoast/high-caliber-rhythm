class_name Palette
extends RefCounted
## One palette for everything on screen. Edgerunners (Trigger) colour language:
## near-black violet ink, acid yellow, cyan, hot magenta, warning red, and a
## cold paper for lit surfaces. Every material and HUD colour references this
## file; the art pass (ASSETS.register.md) changes values here, nowhere else.

const INK := Color(0.043, 0.027, 0.078)
const INK_LINE := Color(0.06, 0.03, 0.09)
const PAPER := Color(0.91, 0.89, 0.94)
const CONCRETE := Color(0.16, 0.17, 0.22)
const STEEL := Color(0.22, 0.20, 0.30)
const BLOCK := Color(0.30, 0.28, 0.40)
const YELLOW := Color(0.953, 0.902, 0.0)
const CYAN := Color(0.0, 0.941, 1.0)
const MAGENTA := Color(1.0, 0.169, 0.839)
const RED := Color(1.0, 0.231, 0.231)
const LIME := Color(0.65, 1.0, 0.25)
const AMBER := Color(1.0, 0.65, 0.10)
const VIOLET := Color(0.45, 0.20, 0.55)

## Weak-point colour by aim kind.
const KIND := {
	"FLICK": MAGENTA,
	"SWITCH": AMBER,
	"TRACK": CYAN,
	"MICRO": LIME,
	"RUSH": YELLOW,
}

## Judgement colours.
const JUDGE := {
	"PERFECT": CYAN,
	"GREAT": LIME,
	"GOOD": YELLOW,
	"EARLY": RED,
	"LATE": RED,
	"WHIFF": Color(0.6, 0.6, 0.7),
	"MISS": RED,
}
