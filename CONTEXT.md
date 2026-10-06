# ASCII Animation

A terminal animation composition consists of one or more configured animations. The same composition can be previewed in the editor or played directly from a command.

## Language

**Preset**:
A named animation type, such as `galaxy` or `text-art`, with its own adjustable options. A preset is not a saved scene.
_Avoid_: Template, saved scene

**Animation instance**:
One configured use of a preset within a scene. A scene can contain several instances of the same preset with different options and placements.
_Avoid_: Preset when referring to one configured animation

**Scene**:
The complete composition of animation instances, including their shared playback settings.
_Avoid_: Preset, animation when referring to the complete composition

**Saved scene**:
A scene saved under a name so it can be reopened, adjusted, or played later. It preserves the complete composition, including all configured animation instances and their shared playback settings.
_Avoid_: Preset, command bookmark

**Effect**:
A visual transformation available within a preset, such as a text wave or typewriter effect. An effect is not independently placed in a scene unless it is offered as a preset in its own right.
_Avoid_: Preset when referring to an option inside another preset

**Placement**:
The region of a scene occupied by an animation instance. Placement describes where an animation appears, not which animation appears in front.
_Avoid_: Layer, depth

**Layer**:
An instance's broad overlap priority: background, normal, or foreground. A layer is not an animation instance or a list of instances.
_Avoid_: Placement, scene item

**Z-index**:
An instance's overlap priority within its layer. Higher values appear in front of lower values in the same layer.
_Avoid_: Layer, placement

**Canvas**:
The logical area in which a scene is composed. It may be larger than the area currently visible in the terminal.
_Avoid_: Terminal size, viewport

**Viewport**:
The visible terminal area presenting part or all of the canvas. A smaller viewport does not imply a smaller composition.
_Avoid_: Canvas, scene size