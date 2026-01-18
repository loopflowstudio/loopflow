# Demo GIF generation and preview

TAPES = demo.tape
GIFS = $(TAPES:.tape=.gif)

.PHONY: all gifs preview clean

all: gifs

# Generate all GIFs (run from repo root)
gifs:
	@echo "Generating demo GIFs..."
	cd .. && vhs docs/demo.tape
	@echo "Done! GIFs are in docs/"

# Preview docs locally
preview:
	@echo "Starting preview server at http://localhost:8000"
	@echo "Open http://localhost:8000/index.html to see the docs"
	python3 -m http.server 8000

# Clean generated files
clean:
	rm -f demo.gif debug-demo.gif design-demo.gif
