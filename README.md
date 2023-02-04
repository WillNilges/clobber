# clobber
A service for determining if GPUs are in use

Have your users made it a habbit of starving eachother's GPU workloads of memory? Do you want that to not happen?

Well, too bad. I'm not there yet. (🚧 This app is still in development 🚧) But, I did create a thingy that will tell your users to be polite!

Clobber will run on login via a profile.d script and detect if anybody is using one of your GPUs.
If they are, they will get a helpful message telling them not to be a dick, and to contact people
if they are causing problems.

| Everything is groovy | Everything is not groovy |
---|---
|![jet_caution](https://user-images.githubusercontent.com/42927786/216744075-c55f0313-028a-44cd-b7af-2fead2f4398a.png)|![jet_warning](https://user-images.githubusercontent.com/42927786/216744078-1aea325a-9b0a-4a3e-8a9d-7a8c73f54364.png)|
