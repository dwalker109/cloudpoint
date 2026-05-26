# Cloudpoint

> Bringing modern cloud save to 3DS!

Cloudpoint allows you to sync all of your saves (and extdata) between all of your 3DS & 2DS devices, 
via a central server. Transfer progress between consoles effortlessly, the way you're probably used 
to from more modern consoles.

## Installing

- Cloudpoint is available on Universal Updater, which is the best way to install it and keep it up to date.
- Alternatively, download the latest release manually and install with FBI.

## Quickstart

- Run Cloudpoint on your first console - it will scan for saves and enable them for auto sync.
- Press (A) to sync and wait for the progress bar to complete.
- Press (R) to reach the *Link* screen and press (X) to send your key to another console
- Run Cloudpoint on your second console - it will scan for saves and enable them for auto sync.
- Press (R) to reach the *Link* screen and press (Y) to receive your key from the first console.
  Cloudpoint will restart on completion.
- Once it reloads, press (A) to sync and expect to be asked to resolve conflicts for any game
  You have installed on both. You will usually see this screen *the first time* you sync a
  game on a given console, or if you progress in a game on *multiple consoles without syncing*.

## Best Practice

- **Keep save backups yourself**. Do this from time to time. Bugs happen and I don't want you to lose
  your 1000 hour Pokémon saves.
- Make a backup of your *user.key* from `/3ds/Cloudpoint/user.key` (you will need this in the event
  you lose your console or memory card, there is no other way to recover your saves).
- Auto sync when you pick up your console for a play session, auto sync again when you finish. This
  will avoid any need to resolve conflicts.

## FAQ

- *I can't see my game in Cloudpoint - where is it?*: Make sure you have run a game at least once to
  initialise the save, and then press (X) to refresh in Cloudpoint - it should then appear.

## Limitations

Cloudpoint can't run in the background, and it can't automatically run when you launch a game. This
isn't something which 3DS can support, so you will need to manually run syncs (see *best practice*).

3DS doesn't provide a method for knowing when a save was last modified, so we can't show that in
the UI. We *do* know when you last synced a save, so we use that in the UI instead.

## Roadmap

- Time travel; move between server save versions at your leisure.
