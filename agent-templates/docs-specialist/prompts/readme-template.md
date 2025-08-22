# README Template for {{project_name}}

This template provides a structure for creating comprehensive README documentation.

## Basic README Structure

````markdown
# {{project_name}}

{{project_description}}

## Features

- Key feature 1
- Key feature 2  
- Key feature 3

## Quick Start

### Prerequisites

List required software, versions, and system requirements.

### Installation

Step-by-step installation instructions.

{{#if (eq framework "rust")}}
```bash
# Clone the repository
git clone https://github.com/your-org/{{project_name}}.git
cd {{project_name}}

# Build the project
cargo build --release

# Run tests
cargo test
```
{{/if}}

{{#if (eq framework "python")}}
```bash
# Clone the repository  
git clone https://github.com/your-org/{{project_name}}.git
cd {{project_name}}

# Create virtual environment
python -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate

# Install dependencies
pip install -r requirements.txt
```
{{/if}}

{{#if (eq framework "nodejs")}}
```bash
# Clone the repository
git clone https://github.com/your-org/{{project_name}}.git
cd {{project_name}}

# Install dependencies
npm install

# Run development server
npm run dev
```
{{/if}}

### Basic Usage

{{#if include_examples}}
Provide a simple example showing the main functionality:

```{{framework}}
// Example code showing basic usage
```
{{else}}
Describe the basic usage without detailed code examples.
{{/if}}

## Documentation

- [User Guide](docs/user-guide.md) - Complete user documentation
- [API Reference](docs/api-reference.md) - Detailed API documentation  
- [Contributing](CONTRIBUTING.md) - Guidelines for contributors
- [Changelog](CHANGELOG.md) - Version history

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

This project is licensed under the [LICENSE NAME](LICENSE) - see the LICENSE file for details.

## Support

- Issues: [GitHub Issues](https://github.com/your-org/{{project_name}}/issues)
- Discussions: [GitHub Discussions](https://github.com/your-org/{{project_name}}/discussions)
````

## Content Guidelines

1. **Keep it scannable**: Use headers, bullet points, and short paragraphs
2. **Lead with value**: Start with what the project does and why it matters
3. **Provide quick wins**: Include a simple example that works immediately  
4. **Link to details**: Use the README as a hub that links to detailed documentation
5. **Maintain freshness**: Keep installation and usage instructions up-to-date