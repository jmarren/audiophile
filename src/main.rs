use rand::RngExt;




struct WavFile<'a> {
    contents: Vec<u8>,
    path: &'a str,
}

#[derive(Copy, Clone)]
struct Sample {
    left: u16,
    right: u16,
}

impl Sample {
    fn to_bytes(&self) -> Vec<u8> {
        [
          self.left.to_le_bytes(),
          self.right.to_le_bytes()
        ]
            .to_vec()
            .into_flattened()
    }

}

struct Samples {
    data: Vec<Sample>
}

impl Samples {
    fn to_bytes(&self) -> Vec<u8> {
        self.data
            .iter()
            .map(| s | { s.to_bytes() })
            .flatten()
            .collect()
    }

    fn join(a: Samples, b: Samples) -> Samples {
        Samples {
            data: a.data.into_iter().chain(b.data).collect()
        }
    }

    fn range(&self, start: usize, end: usize) -> Samples {
        Samples { 
            data: self.data[start..end].to_vec(),
        }
    }

    fn first_half(&self) -> Samples {
        self.range(0, self.data.len() / 2)
    }

    fn second_half(&self) -> Samples {
        self.range(self.data.len() / 2, self.data.len())
    }

    fn split_even(&self) -> (Samples, Samples) {
        (self.first_half(), self.second_half())
    }

    fn doubled_up(mut self) -> Self {
        self.data = self.data
            .iter()
            .flat_map(| x | { vec![*x, *x] })
            .collect();
        self
    }

    fn double_up(&mut self) {
        self.data = self.data
            .iter()
            .flat_map(| x | { vec![*x, *x] })
            .collect();
    }

    fn mute_channel(&mut self, chan: Channel) {
        self.data
            .iter_mut()
            .for_each(| s | {
                match chan {
                    Channel::Left => s.left = 0,
                    Channel::Right => s.right = 0,
                }
            });
    }

}


impl From<&[u8; 4]> for Sample {
    fn from(value: &[u8; 4]) -> Self {
        Sample {
            left: u16::from_le_bytes([value[0], value[1]]),
            right: u16::from_le_bytes([value[2], value[3]]),
        }
    }
}

impl <'a>From<&WavFile<'a>> for Samples {
    fn from(value: &WavFile) -> Self {
        let data_samples = value
            .data_ref()
            .chunks_exact(4)
            .map(| x | {
                let val: [u8; 4] = x.try_into().unwrap();
                Sample::from(&val)
            })
            .collect();
        
        Samples{
            data: data_samples,
        }
    }
}

enum Channel {
    Left,
    Right,
}

#[derive(Debug)]
struct Peak {
    index: usize,
    length: usize,
}

impl<'a> WavFile<'a> {
    
    async fn from_file(path: &'a str) -> WavFile<'a> {
        let file = tokio::fs::read(&path)
                    .await
                    .expect("failed to read audio file");
        WavFile {
            contents: file,
            path: path,
        }
    }

    fn samples(&self) -> Samples {
        Samples::from(self)
    }

    fn header(&self) -> &[u8] {
        &self.contents[..44]
    }

    fn data_ref(&self) -> &[u8] {
        &self.contents[44..]
    }

    fn low_to_high(&mut self)  {
        let samples = self.samples();
        let (mut first, second) = samples.split_even();
        first.double_up();
        let full = Samples::join(first, second);
        self.set_data(full.to_bytes());

    }

    fn double_up_u16s(data: Vec<u16>) -> Vec<u16> {
        let mut new_data = vec![];
        data.iter().for_each(|x| {
                new_data.extend(vec![x, x]);
        });
        new_data
    }
    
    fn merge_channels(left: Vec<u16>, right: Vec<u16>) -> Vec<u8> {
        let mut new_data: Vec<u8> = vec![];
        
        let mut i = 0;
        while i < left.len() && i < right.len() {
            let left_bytes = left[i].to_le_bytes(); 
            let right_bytes = right[i].to_le_bytes(); 
            new_data.extend(left_bytes);
            new_data.extend(right_bytes);
            i += 1;
        }
        new_data
    }

    fn flatten_peaks(&mut self) {

    }

    fn half_freq(&mut self) {
        self.set_data(self.samples().doubled_up().to_bytes());
    }

    fn set_data(&mut self, new_data: Vec<u8>) {
        let mut new_contents = Vec::from(self.header());
        let new_len_bytes = new_data.len().to_le_bytes();
        new_contents.extend(new_data);
        self.contents = new_contents;
        // set the new size within the header
        new_len_bytes
            .iter()
            .enumerate()
            .for_each(|(i, x)| self.contents[40 + i] = *x);
    }


    fn mute_channel(&mut self, chan:Channel) {
        let mut samples = self.samples();
        samples.mute_channel(chan);
        self.set_data(samples.to_bytes());
    }

    fn add_noise(&mut self, val: u16) {
        let mut left = self.channel_u16(Channel::Left);
        let mut right = self.channel_u16(Channel::Right);
        let mut rng = rand::rng();
    
        let mod_factor = u16::MAX / 40;

    
        left.iter_mut().for_each(| x | {
            *x = x.saturating_add(rng.random::<u16>() % mod_factor);
        });
    
        right.iter_mut().for_each(| x | {
            *x = x.saturating_add(rng.random::<u16>() % mod_factor);
        });
    
        self.set_data(Self::merge_channels(left, right));
    }

    
    fn channel_u16(&self, chan: Channel) -> Vec<u16> {
        let mut skip = match chan {
            Channel::Left => false,
            Channel::Right => true,
        };
        self.data_ref()
            .chunks_exact(2)
            .filter_map(|chunk| {
                if skip {
                    skip = false;
                    None
                } else {
                    skip = true;
                    Some(u16::from_le_bytes([chunk[0], chunk[1]]))
                }
            })
            .collect()
    }


    fn data_mut(&mut self) -> &mut [u8] {
        &mut self.contents[44..]
    }
    
    async fn write_to(&self, out_path: &str) {
        tokio::fs::write(&out_path, &self.contents)
            .await
            .expect(format!("failed to wav to {}", out_path).as_str());
    }

    fn double_wrap_data(&mut self) {
        self.data_mut().iter_mut().for_each(| x | {
            *x = x.wrapping_mul(2);
        });
    }


    fn mul_wrap_data(&mut self, factor: u8) {
        self.data_mut().iter_mut().for_each(| x | {
            *x = x.wrapping_mul(factor);
        });
    }


    fn add_wrap_data(&mut self, val: u8) {
        self.data_mut().iter_mut().for_each(| x | {
            *x = x.wrapping_add(val);
        });
    }

    fn find_peaks(&self) {

        let mut curr_highest = u16::MIN;
        let mut potential_peak = false;       
        let mut peaks: Vec<Peak> = vec![];
        let mut curr_len = 0;


        self.channel_u16(Channel::Left).iter().enumerate().for_each(|(i, x)| {
            if *x > curr_highest {
                // println!("{} greater than {curr_highest}", *x);
                potential_peak = true;       
                curr_highest = *x;       
            } else {
                if potential_peak {

                    let last_i = match peaks.last() {
                        Some(p) => p.index,
                        None => 1,
                    };
                    peaks.push(Peak{
                        index: i,
                        length: i - last_i,
                    });
                    curr_len = 0;
                    // peak_indexes.push(i);
                    potential_peak = false;
                    curr_highest = *x;
                } else {
                    curr_highest = *x;
                    curr_len += 1;
                }
            }
        });
        
        let lengths: Vec<usize> = peaks.iter().map(| p | { p.length }).collect();
        println!("{:?}", lengths);
    
        // println!("{:?}", peaks);
    }

}


async fn process_file() {
    
    let mut wav_file = WavFile::from_file("./Viola-C5.wav").await;
        
    wav_file.low_to_high();
    // wav_file.mute_channel(Channel::Right);

    wav_file.write_to("new_file.wav").await;
}


#[tokio::main]
async fn main() {
    process_file().await;
}


    // let  file = tokio::fs::read("./Viola-C5.wav")
    //                 .await
    //                 .expect("failed to read audio file");
    //
    // println!("{:?}", file);
    //
    // let factor: u8 = 2;
    //
    // let data = &file[44..];
    //
    // let mut modified_data: Vec<u8> = data.iter().map(| x | {
    //     x.wrapping_mul(factor)
    // }).collect();
    //
    // let mut out: Vec<u8> = vec![0; 44];
    //
    // out.clone_from_slice(&file[0..44]);
    //
    // out.append(&mut modified_data);
    //
    // let _ = tokio::fs::write("./new-viola-c5.wav", &out).await.expect("failed to write new file");
    
