# Contributing

Contributing to the Usagi book is pretty easy! It's all written in Markdown and
hosted on Codeberg in the main Usagi repo:
[https://codeberg.org/brettchalupa/usagi/src/branch/main/book](https://codeberg.org/brettchalupa/usagi/src/branch/main/book)

If you find typos, fixes would be much appreciated. If something is unclear and
you want to revise the explanation, that'd be great!

You can
[open an issue on Codeberg](https://codeberg.org/brettchalupa/usagi/issues/new)
if you want to discuss something or share feedback publicly.

Adding tutorial chapters to the book is quite involved. Each block of code is
loaded from a real working Usagi project. So for each section in the book,
there's a full Usagi game where various snippets are loaded from. For example,
see the Shoot 'Em Up code:
[https://codeberg.org/brettchalupa/usagi/src/branch/main/book/src/code/02-shoot-em-up](https://codeberg.org/brettchalupa/usagi/src/branch/main/book/src/code/02-shoot-em-up).
This helps ensure that the code included in the book works. But it also means
making changes is pretty brittle because adding lines of code or removing them
shifts the line numbers around. Basically, in summary, it's a bit of a pain if
you aren't used to it.

Adding recipe chapters, which are basically blog posts that are less intense
than tutorials, are welcome! If you have a topic you want to write about, submit
a PR and we can review and discuss there. Or we can talk about it beforehand if
you want.

Your writing doesn't have to match my voice, but it should be clear, concise,
and the code should be correct.

**Absolutely NO text or code from AIs/LLMs should be added to the book. Any
hints of AI contributions will be immediately rejected and no further
contributions will be accepted.**

View the README for more details on viewing the book locally:
[https://codeberg.org/brettchalupa/usagi/src/branch/main/book#_game-programming-with-usagi_](https://codeberg.org/brettchalupa/usagi/src/branch/main/book#_game-programming-with-usagi_)

The book is formatted with `just fmt` when in the `book` directory. It uses Deno
to format all the Markdown to make it look nice.
