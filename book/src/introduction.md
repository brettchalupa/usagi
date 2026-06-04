# Introduction

**NOTE**: this book is a work in progress!

Hi, my name is Brett Chalupa! I'm the creator of Usagi Engine and primary author
of _Game Programming with Usagi_. This book is a guide for how to jump into
making games. It's project-based and starts with the fundamentals of
programming. This book, much like Usagi itself, is simple, accessible, and free.

The advice often given to new game developers is to make a lot of small games.
That way the new developer can learn the fundamentals of game development,
explore ideas, and figure out what games they enjoy making. The problem though
is that most game engines aren't conducive to making small games quickly. After
making small games for over twenty years, I decided to try to solve that problem
by creating **Usagi Engine**, a free and open source game engine that's
specifically focused on making small 2D games and being able to quickly share
them. Your code, art assets, audio files, and data are live reloaded as you
change them, removing the laborious step of having to constantly re-launch your
game after every change to test it. If want to see if a different color looks
better, just edit your sprite, save it, and see it update instantly. I can't
overstate how useful this is.

Usagi games are programmed with Lua, a simple and widely-used language. There
are many different game engines and libraries that use Lua, which means the
knowledge you gain from learning to make games with Usagi is useful even if you
stop using the engine. When you code a game, the engine provides functions,
which are named pieces of code that do _something_ like draw a shape on the
screen or play a sound effect. Large game engines have hundreds or thousands of
functions, requiring you to study complex manuals to find what you need. Usagi,
on the other hand, is embraces constraints and has a limited number of functions
that cover the functionality most games need.

Usagi is not the everything engine. There's a lot it doesn't do. But it excels
at being simple, approachable, and fast. When you're exploring an idea or
participating in a game jam, you don't want to spend your time coding input
mapping or where to put save data on Linux computers. You want to focus on
making your game fun to play. Usagi provides input mapping, simple ways to check
for player input via keyboard and gamepad, easy save data, and a fully-featured
Pause menu. Also, with a single command you can export your game for web, Linux,
macOS, and Windows.

## Book Overview

This book, _Game Programming with Usagi Engine_, is written for someone just
getting started out making games. If you've coded games before, great! You'll be
able to pick up on things even quicker. But if you haven't, don't worry. We'll
go through making games step-by-step in guided tutorials. The second half of the
book contains recipes that are focused lessons on specific functionality.

There are two sections to the book: **Tutorials** and **Recipes**.

The **Tutorials** are guided, project-based lessons where you'll make a game
from scratch. The complexity of each chapter steadily grows, so if you're new to
game programming, it's best to start at the beginning and go through each one in
order. If you've programmed games before, jump around a bit!

The **Recipes** are deep-dives into specific topics. Sort like blog posts on
various topics related to Usagi Engine. They're great for learning a specific
concept as opposed to programming and engine fundamentals.

## Getting Started

There are three things you need to get started with Usagi:

1. A computer running Linux, macOS, or Windows.
2. A text editor installed for writing code. I like [Zed](https://zed.dev), a
   free and open source editor. Visual Studio Code is another popular free code
   editor.
3. Usagi installed; follow the instructions at
   [usagiengine.com](https://usagiengine.com#install).

Usagi is interacted with via the command line. You type in commands rather than
click buttons in a graphical user interface. On Linux and macOS, this program is
called the Terminal. On Windows, the two primary tools are called the Command
Prompt and PowerShell. While the command line can daunting at first, there are
only a few commands you'll need to know to work with Usagi most of the time.

We'll cover all of the essential commands and make our first basic game in the
upcoming chapters.
