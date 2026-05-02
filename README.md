# Tinteán

**A Home Automation framework designed for reliability and customisability**

This is not a complete home automation system, this merely provides the tools needed to create such systems.
That is to say that this does not produce a binary, rather you need to create your own project depending on this one 
to configure your system. I may add a lightweight wrapper for doing so in the future

My intention is to create a generic framework that I can use to create a system tailored to my home, I may extend this
in the future, but for now that is the goal of this project

This project in inspired by [Home Assistant](https://www.home-assistant.io/), if you're looking for a more 
feature-complete user-friendly system, I encourage you to check out Home Assistant

This project started out of my frustrations with Home Assistant, such as:
- Updates frequently broke parts of the system since so much was in separate (often third-party) components
which can easily become incompatible with one another in subtle ways
- Doing anything truly custom felt like the system was fighting you rather than making it easy

I do aim to allow third-party components for this system, but they will bve compiled in rather than included at run-time

### Project Priorities:
#### Stability
If you have a working running instance, it should keep running seamlessly without any action needed by the user

#### Compile-time Guarantees
**If it compiles it should work**. 
Rust is already very good at accomplishing this and this project aims to use the rust
type system to fully ensure this as much as possible.

Potential sourced of runtime issues (e.g. Connection issues) should result in clear `Result::Err` values which can be
used to handle things gracefully

### Project name

Tinteán (Irish): Hearth/Fireplace, culturally associated with home/comfort/fireside

Inspired by the seanfhocal (proverb) "Níl aon tinteán mar do thinteán féin" = "There's no place like home", literally: "There's no hearth like your own hearth"

Pronunciation (apx): (/ˈtʲɪn̠ʲtɔːn/) tin - tawn (rhythms with fawn)

Fada (á): The fada over the _'a'_ is an important part of the word, without that it is not a valid Irish word, however 
since I don't want to be facing potential issues with keyboard compatability for anyone who doesn't know how to type 
a fada or any potential software issues, it will be excluded in any code including the repository name

### TODO: add documentation on:
* Concepts: devices, automations, etc.
* How to set up
* How to define devices
* How to define automations
* How to define integrations
* How to use integrations including build-in ones