dev:
    mdbook serve

fmt:
    deno fmt *.md src/*.md src/recipes/*.md
 
deploy:
    mdbook build
    npx netlify deploy --prod --dir book
