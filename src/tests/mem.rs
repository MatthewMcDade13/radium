#[cfg(test)]
mod tests {
    use anyhow::bail;

    use crate::sys::mem::{BumpAllocator, StackAllocator};

    struct Point {
        x: f64,
        y: f64,
    }

    #[test]
    fn stack() -> anyhow::Result<()> {
        let mut sa = StackAllocator::<4096>::new();
        {
            let x = sa.alloc(4)?;
            let p = sa.alloc(Point { x: 56.0, y: 69. })?;
            let y = sa.alloc(56.)?;

            assert_eq!(4, *x);
            assert_eq!(56., *y);
            assert_eq!(p.x, 56.0);
            assert_eq!(p.y, 69.0);

            sa.clear();
        }

        let x = sa.alloc(String::from("aye lmao"))?;
        assert_eq!(*x, "aye lmao");
        Ok(())
    }

    #[test]
    fn bump() -> anyhow::Result<()> {
        let mut ba = BumpAllocator::new(4096)?;

        {
            let x = ba.alloc(4)?;
            let p = ba.alloc(Point { x: 56.0, y: 69. })?;
            let y = ba.alloc(usize::MAX)?;

            assert_eq!(4, *x);
            assert_eq!(usize::MAX, *y);
            assert_eq!(p.x, 56.0);
            assert_eq!(p.y, 69.0);

            ba.clear();
        }

        let x = ba.alloc(String::from("aye lmao"))?;
        assert_eq!(*x, "aye lmao");

        Ok(())
    }

    #[test]
    fn stack_overflow() -> anyhow::Result<()> {
        let mut sa = StackAllocator::<4096>::new();

        for i in 0..500 {
            let x = i as f64;
            match sa.alloc(Point { x: x * 2., y: x }) {
                Ok(_) => {}
                Err(_) => return anyhow::Result::Ok(()),
            }
        }
        sa.clear();
        bail!("StackAllocator did not fail after 500 entries of struct Point with Allocator buffer size of 4096");
    }
}
