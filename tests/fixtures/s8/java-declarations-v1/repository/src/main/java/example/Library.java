package example;
import java.util.List;
import static java.lang.Math.PI;

// class CommentDecoy { void fake() {} }
@Deprecated
public class Library<T> extends Base implements Runnable {
    private final String name = "class StringDecoy {}";
    int first = 1, second[];
    public Library(String name) { this.name = name; }
    public <U> List<U> convert(U input) {
        class LocalDecoy {}
        return List.of(input);
    }
    public void run() {}
    public void overloaded(int value) {}
    public void overloaded(String value) {}
    public interface Nested { String label(); }
    public enum Status {
        READY,
        BUSY { public String toString() { return "busy"; } };
        private final int code = 1;
    }
    public record Pair(String left, int right) {
        public Pair { if (left == null) throw new IllegalArgumentException(); }
    }
    public @interface Marker { String value() default "sample"; }
    static { class InitializerDecoy {} }
}
class Base {}
