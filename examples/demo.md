# Markdown extension demo

**Bold**, *emphasis*, and `inline code` use Zed's built-in Markdown colors.

## Java without the Java extension

```java
// This is text, never executed.
public class Demo {
    public static void main(String[] args) {
        System.out.println("Hello 👋");
    }
}
```

### A nested section

Folding the preceding level-two heading includes this section.

```python
def greet(name):
    return f"Hello {name}"
```

## Inert inputs

```sh
echo 'No commands are run by this extension'
```

````text
# This is not a heading
```java
public class LiteralExample {}
```
````

> # This quoted heading does not end the section

Unknown fence labels remain plain text:

```unknown-language
anything here
```

Last section
============

An underlined heading also supports folding.
