/// 4-точечная кубическая интерполяция Catmull-Rom (гладкая кривая без металлического роботизированного звона)
#[inline]
fn interpolate_cubic(y0: f32, y1: f32, y2: f32, y3: f32, t: f32) -> f32 {
    let c0 = y1;
    let c1 = 0.5 * (y2 - y0);
    let c2 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
    let c3 = 0.5 * (y3 - y0) + 1.5 * (y1 - y2);
    ((c3 * t + c2) * t + c1) * t + c0
}

/// Потоковый ресемплер для ввода с фильтрацией артефактов
pub struct InputResampler {
    from_rate: f64,
    to_rate: f64,
    pos: f64,
    y: [f32; 4],
}

impl InputResampler {
    pub fn new(from_rate: u32, to_rate: u32) -> Self {
        Self {
            from_rate: from_rate as f64,
            to_rate: to_rate as f64,
            pos: 0.0,
            y: [0.0; 4],
        }
    }

    pub fn process<F: FnMut(f32)>(&mut self, sample: f32, mut emit: F) {
        // Сдвигаем буфер истории сэмплов
        self.y[0] = self.y[1];
        self.y[1] = self.y[2];
        self.y[2] = self.y[3];
        self.y[3] = sample;

        let step = self.from_rate / self.to_rate;
        while self.pos < 1.0 {
            let interpolated =
                interpolate_cubic(self.y[0], self.y[1], self.y[2], self.y[3], self.pos as f32);
            emit(interpolated);
            self.pos += step;
        }
        self.pos -= 1.0;
    }
}

/// Потоковый ресемплер для вывода с защитой от щелчков
pub struct OutputResampler {
    from_rate: f64,
    to_rate: f64,
    pos: f64,
    y: [f32; 4],
    initialized: bool,
}

impl OutputResampler {
    pub fn new(from_rate: u32, to_rate: u32) -> Self {
        Self {
            from_rate: from_rate as f64,
            to_rate: to_rate as f64,
            pos: 0.0,
            y: [0.0; 4],
            initialized: false,
        }
    }

    pub fn next_sample(&mut self, mut pull_src: impl FnMut() -> f32) -> f32 {
        if !self.initialized {
            self.y[0] = pull_src();
            self.y[1] = pull_src();
            self.y[2] = pull_src();
            self.y[3] = pull_src();
            self.initialized = true;
        }

        let step = self.from_rate / self.to_rate;
        while self.pos >= 1.0 {
            self.pos -= 1.0;
            self.y[0] = self.y[1];
            self.y[1] = self.y[2];
            self.y[2] = self.y[3];
            self.y[3] = pull_src();
        }

        let interpolated =
            interpolate_cubic(self.y[0], self.y[1], self.y[2], self.y[3], self.pos as f32);
        self.pos += step;
        interpolated
    }
}
