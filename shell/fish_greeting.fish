# Pokefetch as your Fish greeting.
#
# Install by linking this file into Fish's autoload directory:
#
#     ln -s (pwd)/shell/fish_greeting.fish ~/.config/fish/functions/fish_greeting.fish
#
# Fish autoloads a function from ~/.config/fish/functions/<name>.fish the first
# time that name is called, so the file must be named after the function it
# defines. Defining `fish_greeting` replaces Fish's built-in welcome message --
# this is the supported way to change it, rather than printing from config.fish.
#
# Fish calls this once per interactive shell and never for scripts, so there is
# no `status is-interactive` guard here: reaching this function already means
# the shell is interactive.

function fish_greeting --description 'Greet with a Pokemon and a machine summary'
    # `command --query` checks PATH without executing anything. Without this a
    # shell on a machine where Pokefetch is not installed would open with an
    # error instead of a prompt.
    command --query pokefetch; or return

    # Escape hatch, useful in a screen share or a recording:
    #     set --universal --export POKEFETCH_NO_GREETING 1
    set --query POKEFETCH_NO_GREETING; and return

    pokefetch greet
end
