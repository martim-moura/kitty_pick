# Optional: source this file after putting kitty-pick on PATH.
# Run inline, not in an overlay: a child cannot change this shell's cwd.
function kitty-pick-cd --description 'Browse with skim and cd in the calling Fish shell'
    set -l selected (command kitty-pick choose-dir $argv | string split0)
    if test (count $selected) -eq 1
        cd -- "$selected[1]"
    else
        return 1
    end
end
