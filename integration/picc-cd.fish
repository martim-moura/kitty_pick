# Optional: source this file after putting picc-menu on PATH.
# Run inline, not in an overlay: a child cannot change this shell's cwd.
function picc-cd --description 'Browse with skim and cd in the calling Fish shell'
    set -l selected (command picc-menu choose-dir $argv | string split0)
    if test (count $selected) -eq 1
        cd -- "$selected[1]"
    else
        return 1
    end
end
