use ika::Pool;

fn main() {
    let mut str_pool: Pool<String> = Pool::new(10);
    println!("available {:?}", str_pool.available());
    println!("capacity  {:?}", str_pool.capacity());
    println!("{:?}", str_pool);
    println!();

    str_pool.spawn_some(5)
            .drain(..)
            .enumerate()
            .for_each(| (i, r) | {
                r.push_str(&i.to_string());
                r.push_str(" hallo");
            });
    println!("available {:?}", str_pool.available());
    println!("capacity  {:?}", str_pool.capacity());
    println!("{:?}", str_pool);
    println!();

    let ok = str_pool.detach(2);
    println!("{:?}", ok);
    println!();

    println!("available {:?}", str_pool.available());
    println!("capacity  {:?}", str_pool.capacity());
    println!("{:?}", str_pool);
    println!();

    str_pool.attach(2, "wowo".to_owned());

    println!("available {:?}", str_pool.available());
    println!("capacity  {:?}", str_pool.capacity());
    println!("{:?}", str_pool);
    println!();
}
