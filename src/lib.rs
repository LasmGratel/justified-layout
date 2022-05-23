use std::cell::RefCell;
use std::rc::Rc;

#[cfg(test)]
mod tests {
    use crate::JustifiedLayout;

    #[test]
    fn it_works() {
        let mut layout = JustifiedLayout::default();
        let computed = layout.compute_layout_by_ratio([0.5f64, 1.5, 1.0, 1.8, 0.4, 0.7, 0.9, 1.1, 1.7, 2.0, 2.1].into_iter());
        println!("{:?}", computed);
        let last_box = computed.boxes[10].as_ref().borrow();
        assert_eq!(last_box.aspect_ratio, 2.1);
        assert_eq!(last_box.height, 251.0);
        assert_eq!(last_box.left, 523.0);
        assert_eq!(last_box.top, 1008.0);
        assert_eq!(last_box.width, 527.0);
    }
}

type ItemContainer = Rc<RefCell<LayoutItem>>;

#[derive(Debug)]
pub struct JustifiedLayout {
    pub container_height: f64,
    pub layout_items: Vec<ItemContainer>,
    pub rows: Vec<Row>,
    pub config: Config,
}

impl Default for JustifiedLayout {
    fn default() -> Self {
        let config = Config::default();
        JustifiedLayout {
            container_height: config.container_padding.top,
            config,
            rows: Vec::default(),
            layout_items: Vec::default(),
        }
    }
}

impl JustifiedLayout {
    pub fn new(config: Config) -> Self {
        JustifiedLayout {
            config,
            ..Default::default()
        }
    }

    pub fn create_row(&self) -> Row {
        let mut is_breakout_row = false;
        if let Some(cadence) = self.config.full_width_breakout_row_cadence {
            if (self.rows.len() as u64 + 1) % cadence == 0 {
                is_breakout_row = true;
            }
        }

        let mut row = Row {
            top: self.container_height,
            left: self.config.container_padding.left,
            width: self.config.container_width as f64 - self.config.container_padding.left - self.config.container_padding.right,
            spacing: self.config.box_spacing.horizontal,
            target_row_height: self.config.target_row_height,
            target_row_height_tolerance: self.config.target_row_height_tolerance,
            edge_case_min_row_height: 0.5 * self.config.target_row_height,
            edge_case_max_row_height: 2.0 * self.config.target_row_height,
            is_breakout_row,
            layout_style: self.config.layout_style,
            ..Default::default()
        };
        row.init();
        row
    }

    pub fn push_row(&mut self, row: Row) -> Vec<ItemContainer> {
        self.container_height += row.height + self.config.box_spacing.vertical;

        self.rows.push(row);
        let row = self.rows.last_mut().unwrap();
        let vec = row.items.clone();
        vec.iter().for_each(|item| self.layout_items.push(item.clone()));
        vec
    }

    pub fn compute_layout_by_ratio<I: Iterator<Item = f64>>(&mut self, boxes: I) -> ComputedLayout<> {
        self.compute_layout(boxes.map(|b| LayoutItem::from_ratio(b)))
    }

    pub fn compute_layout<I: Iterator<Item = LayoutItem>>(&mut self, items: I) -> ComputedLayout<> {
        let mut laid_out_items: Vec<ItemContainer> = vec![];
        let mut items: Vec<LayoutItem> = items.collect();
        if let Some(force_ratio) = self.config.force_aspect_ratio {
            items.iter_mut().for_each(|item| item.force_aspect_ratio = Some(force_ratio));
        }

        let mut current_row: Option<Row> = None;
        let mut item_added: bool;
        for data in items.into_iter() {
            if data.aspect_ratio == f64::NAN {
                panic!("Item {:?} has an invalid aspect ratio", data);
            }

            if current_row.is_none() {
                current_row = Some(self.create_row());
            }

            {
                let current_row = current_row.as_mut().unwrap();
                item_added = current_row.add(data.clone());
                if !current_row.is_layout_complete() {
                    continue;
                }
            }

            self.push_row(current_row.take().unwrap()).into_iter().for_each(|item| laid_out_items.push(item));

            if self.rows.len() >= self.config.max_rows {
                break;
            }

            current_row = Some(self.create_row());

            // Item was rejected; add it to its own row
            if item_added {
                continue;
            }

            {
                let current_row = current_row.as_mut().unwrap();
                item_added = current_row.add(data);
                if !current_row.is_layout_complete() {
                    continue;
                }
            }

            self.push_row(current_row.take().unwrap()).into_iter().for_each(|item| laid_out_items.push(item));

            if self.rows.len() >= self.config.max_rows {
                break;
            }

            current_row = Some(self.create_row());
        }


        // Handle any leftover content (orphans) depending on where they lie
        // in this layout update, and in the total content set.
        if let Some(row) = current_row.as_mut() {
            if row.items.len() > 0 && self.config.show_widows {
                // Last page of all content or orphan suppression is suppressed; lay out orphans.
                if let Some(last_row) = self.rows.last() {
                    // Only Match previous row's height if it exists and it isn't a breakout row
                    let next_to_last_row_height = if last_row.is_breakout_row {
                        last_row.target_row_height
                    } else {
                        last_row.height
                    };

                    row.force_complete(false, Some(next_to_last_row_height));
                } else {
                    // ...else use target height if there is no other row height to reference.
                    row.force_complete(false, None);
                }

                self.config.widow_count = row.items.len();
                self.push_row(current_row.take().unwrap()).into_iter().for_each(|item| laid_out_items.push(item));

            }
        }

        // We need to clean up the bottom container padding
        // First remove the height added for box spacing
        self.container_height -= self.config.box_spacing.vertical;

        // Then add our bottom container padding
        self.container_height += self.config.container_padding.bottom;

        ComputedLayout {
            height: self.container_height,
            widow_count: self.config.widow_count,
            boxes: self.layout_items.clone()
        }
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct Padding<T: Default> {
    pub left: T,
    pub right: T,
    pub top: T,
    pub bottom: T,
}

impl<T> From<T> for Padding<T> where T: Copy + Default {
    fn from(value: T) -> Self {
        Padding {
            left: value,
            right: value,
            top: value,
            bottom: value,
        }
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct Spacing<T: Default> {
    pub horizontal: T,
    pub vertical: T,
}

impl<T> From<T> for Spacing<T> where T: Copy + Default {
    fn from(value: T) -> Self {
        Spacing {
            horizontal: value,
            vertical: value,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum WidowLayoutStyle {
    Left, Justify, Center
}

impl Default for WidowLayoutStyle {
    fn default() -> Self {
        WidowLayoutStyle::Left
    }
}

#[derive(Debug)]
pub struct Config {
    pub container_width: u64,
    pub container_padding: Padding<f64>,
    pub box_spacing: Spacing<f64>,
    pub target_row_height: f64,
    pub target_row_height_tolerance: f64,
    pub max_rows: usize,
    pub force_aspect_ratio: Option<f64>,
    pub show_widows: bool,
    pub full_width_breakout_row_cadence: Option<u64>,
    pub layout_style: WidowLayoutStyle,
    pub widow_count: usize,
}

impl Default for Config {
    fn default() -> Self {
        Config::new()
    }
}

impl Config {
    pub fn new() -> Self {
        Config {
            container_width: 1060,
            container_padding: 10.0.into(),
            box_spacing: 10.0.into(),
            target_row_height: 320.0,
            target_row_height_tolerance: 0.25,
            max_rows: usize::MAX,
            force_aspect_ratio: None,
            show_widows: true,
            full_width_breakout_row_cadence: None,
            layout_style: WidowLayoutStyle::Justify,
            widow_count: 0
        }
    }
}

#[derive(Debug)]
pub struct ComputedLayout {
    pub height: f64,
    pub widow_count: usize,
    pub boxes: Vec<Rc<RefCell<LayoutItem>>>
}

#[derive(Debug, Default)]
pub struct Row {
    pub items: Vec<ItemContainer>,

    pub left: f64,
    pub top: f64,

    pub width: f64,
    pub height: f64,

    pub spacing: f64,

    pub target_row_height: f64,
    pub target_row_height_tolerance: f64,

    pub min_aspect_ratio: f64,
    pub max_aspect_ratio: f64,

    pub edge_case_min_row_height: f64,
    pub edge_case_max_row_height: f64,

    pub layout_style: WidowLayoutStyle,
    pub is_breakout_row: bool,
}

impl Row {
    pub fn init(&mut self) {
        self.min_aspect_ratio = self.width / self.target_row_height * (1.0 - self.target_row_height_tolerance);
        self.max_aspect_ratio = self.width / self.target_row_height * (1.0 + self.target_row_height_tolerance);
    }

    pub fn is_layout_complete(&self) -> bool {
        self.height > 0.0
    }

    pub fn add(&mut self, item: LayoutItem) -> bool {
        let item = Rc::new(RefCell::new(item));
        let new_items = std::iter::once(item.clone()).chain(self.items.clone());

        let row_width_without_spacing = self.width - self.items.len() as f64 * self.spacing;
        let new_aspect_ratio = new_items.fold(0.0, |sum, item| item.as_ref().borrow().aspect_ratio + sum);
        let target_aspect_ratio = row_width_without_spacing / self.target_row_height;

        if self.is_breakout_row {
            if self.items.is_empty() {
                if item.as_ref().borrow().aspect_ratio >= 1.0 {
                    let item_aspect_ratio = item.as_ref().borrow().aspect_ratio;
                    self.items.push(item.clone());
                    self.complete_layout(row_width_without_spacing / item_aspect_ratio, Some(WidowLayoutStyle::Justify));
                    return true;
                }
            }
        }

        if new_aspect_ratio < self.min_aspect_ratio {
            self.items.push(item.clone());
            return true;
        }

        if new_aspect_ratio > self.max_aspect_ratio {
            if self.items.is_empty() {
                self.items.push(item.clone());
                self.complete_layout(row_width_without_spacing / new_aspect_ratio, Some(WidowLayoutStyle::Justify));
                return true;
            }

            let previous_row_width_without_spacing = self.get_row_width_without_spacing();
            let previous_aspect_ratio = self.items.iter().fold(0.0, |sum, item| item.as_ref().borrow().aspect_ratio + sum);
            let previous_target_aspect_ratio = previous_row_width_without_spacing / self.target_row_height;

            if (new_aspect_ratio - target_aspect_ratio).abs() > (previous_aspect_ratio - previous_target_aspect_ratio).abs() {
                self.complete_layout(previous_row_width_without_spacing / previous_aspect_ratio, Some(WidowLayoutStyle::Justify));
                return false;
            }

            self.items.push(item.clone());
            self.complete_layout(row_width_without_spacing / new_aspect_ratio, Some(WidowLayoutStyle::Justify));
            return true;
        }

        self.items.push(item.clone());
        self.complete_layout(row_width_without_spacing / new_aspect_ratio, Some(WidowLayoutStyle::Justify));
        true
    }

    #[inline]
    fn get_row_width_without_spacing(&self) -> f64 {
        self.width - (self.items.len() - 1) as f64 * self.spacing
    }

    pub fn complete_layout(&mut self, new_height: f64, layout_style: Option<WidowLayoutStyle>) {
        let mut item_width_sum = self.left;
        let row_width_without_spacing = self.get_row_width_without_spacing();
        let layout_style = layout_style.unwrap_or(WidowLayoutStyle::Justify);

        // TODO Not Rounded
        let new_height = new_height.round();
        let clamped_height = self.edge_case_min_row_height.max(new_height.min(self.edge_case_max_row_height));

        let clamped_to_native_ratio: f64;

        if new_height != clamped_height {
            self.height = clamped_height;
            clamped_to_native_ratio = row_width_without_spacing / clamped_height / (row_width_without_spacing / new_height);
        } else {
            self.height = new_height;
            clamped_to_native_ratio = 1.0;
        }

        self.items.iter_mut().for_each(|item| {
            let mut item = item.as_ref().borrow_mut();
            item.top = self.top;
            item.height = self.height;

            // TODO Not Rounded
            item.width = (item.aspect_ratio * self.height * clamped_to_native_ratio).round();

            // Left-to-right.
            // TODO right to left
            // item.left = this.width - itemWidthSum - item.width;
            item.left = item_width_sum;

            // Increment width.
            item_width_sum += item.width + self.spacing;
        });

        match layout_style {
            WidowLayoutStyle::Left => {}
            WidowLayoutStyle::Justify => {
                item_width_sum -= self.spacing + self.left;

                let error_width_per_item = (item_width_sum - self.width) / self.items.len() as f64;
                let rounded_cumulative_errors: Vec<f64> = self.items.iter().enumerate().map(|(i, _)| ((i + 1) as f64 * error_width_per_item).round()).collect();

                if self.items.len() == 1 {
                    self.items[0].as_ref().borrow_mut().width -= error_width_per_item.round();
                } else {
                    self.items.iter_mut().enumerate().for_each(|(i, item)| {
                        let mut item = item.as_ref().borrow_mut();
                        if i > 0 {
                            item.left -= rounded_cumulative_errors[i - 1];
                            item.width -= rounded_cumulative_errors[i] - rounded_cumulative_errors[i - 1];
                        } else {
                            item.width -= rounded_cumulative_errors[i];
                        }
                    });
                }
            }
            WidowLayoutStyle::Center => {
                let center_offset = (self.width - item_width_sum) / 2.0;
                self.items.iter_mut().for_each(|item| item.as_ref().borrow_mut().left += center_offset + self.spacing);
            }
        }
    }

    pub fn force_complete(&mut self, fit_to_width: bool, row_height: Option<f64>) {
        // TODO Handle fitting to width
        // var rowWidthWithoutSpacing = this.width - (this.items.length - 1) * this.spacing,
        // 	currentAspectRatio = this.items.reduce(function (sum, item) {
        // 		return sum + item.aspectRatio;
        // 	}, 0);
        let row_width_without_spacing = self.width - (self.items.len() - 1) as f64 * self.spacing;
        let current_aspect_ratio: f64 = self.items.iter().map(|item| item.as_ref().borrow().aspect_ratio).sum();
        if let Some(row_height) = row_height {
            self.complete_layout(row_height, Some(WidowLayoutStyle::Left));
        } else if fit_to_width {
            self.complete_layout(row_width_without_spacing / current_aspect_ratio, None);
        } else {
            self.complete_layout(self.target_row_height, Some(WidowLayoutStyle::Left));
        }

    }
}

#[derive(Debug, Default, Clone)]
pub struct LayoutItem {
    pub aspect_ratio: f64,
    pub force_aspect_ratio: Option<f64>,
    pub top: f64,
    pub left: f64,
    pub width: f64,
    pub height: f64,
}

impl LayoutItem {
    pub fn from_ratio(aspect_ratio: f64) -> Self {
        LayoutItem {
            aspect_ratio,
            force_aspect_ratio: None,
            top: 0.0,
            left: 0.0,
            width: 0.0,
            height: 0.0,
        }
    }

    pub fn new(width: f64, height: f64) -> Self {
        LayoutItem {
            aspect_ratio: width / height,
            force_aspect_ratio: None,
            top: 0.0,
            left: 0.0,
            width: 0.0,
            height: 0.0,
        }
    }
}