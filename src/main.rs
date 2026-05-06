slint::slint! {
    import { Button,VerticalBox } from "std-widgets.slint";
    export component App inherits Window {
        in property <int> counter: 1;
        callback clicked <=> btn.clicked;
        VerticalBox {
            Text {
                text: "Hello, World";
            }
            btn := Button {
                text: "yaya";
            }
        }
    }
}
fn main() {
    let app = App::new().unwrap();
    let weak = app.as_weak();
    app.on_clicked(move || {
        if let Some(app) = weak.upgrade() {
            app.set_counter(app.get_counter() + 2);
        }
    });
    app.run().unwrap();
    println!("Hello, world!");
}
