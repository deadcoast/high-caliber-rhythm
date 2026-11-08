extends CanvasLayer
## Any Song, step one: pick a file. O opens the picker; the driver analyses
## the file and the confirm screen (song_confirm.gd) takes over. Levels built
## this way land in user://songs and play like any level JSON.

var dialog: FileDialog
var status: Label


func _ready() -> void:
	layer = 3
	status = Label.new()
	status.set_anchors_preset(Control.PRESET_CENTER)
	status.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	status.add_theme_font_size_override("font_size", 28)
	status.add_theme_color_override("font_color", Palette.CYAN)
	status.visible = false
	add_child(status)
	dialog = FileDialog.new()
	dialog.title = "Any Song"
	dialog.file_mode = FileDialog.FILE_MODE_OPEN_FILE
	dialog.access = FileDialog.ACCESS_FILESYSTEM
	dialog.filters = PackedStringArray(["*.wav, *.flac, *.ogg, *.mp3, *.m4a, *.aac ; Audio"])
	dialog.use_native_dialog = true
	dialog.size = Vector2i(900, 600)
	dialog.file_selected.connect(_on_file)
	dialog.canceled.connect(func() -> void: Input.mouse_mode = Input.MOUSE_MODE_CAPTURED)
	add_child(dialog)


func _process(_delta: float) -> void:
	if DB.analysing != status.visible:
		status.visible = DB.analysing
		status.text = "LISTENING   " + DB.analysing_name
		status.reset_size()
		status.position = (get_viewport().get_visible_rect().size - status.size) * 0.5


func _unhandled_input(event: InputEvent) -> void:
	if event.is_action_pressed("any_song") and not DB.analysing:
		open()
		get_viewport().set_input_as_handled()


func open() -> void:
	Input.mouse_mode = Input.MOUSE_MODE_VISIBLE
	dialog.popup_centered()


func _on_file(path: String) -> void:
	DB.analyse_song(path)
