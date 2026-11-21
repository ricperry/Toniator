#!/usr/bin/env python3
"""Quantify native artwork differences and retain unflattened100% crops/scalar diagnostics."""
from pathlib import Path
import hashlib
import json
import sys
import numpy as np
from PIL import Image,ImageDraw,ImageFont

ROOT=Path.cwd()


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def distribution(values):
    a=np.asarray(values)
    return {'mean':float(a.mean()),'p50':float(np.quantile(a,.5)),'p90':float(np.quantile(a,.9)),'p95':float(np.quantile(a,.95)),'p99':float(np.quantile(a,.99)),'max':float(a.max())}


def linear(rgb):
    return np.where(rgb<=.04045,rgb/12.92,((rgb+.055)/1.055)**2.4)


def analyze(cpu_dir,gpu_dir,out):
    cpu_path=Path(cpu_dir).resolve();gpu_path=Path(gpu_dir).resolve()
    cpu_image=Image.open(cpu_path).convert('RGBA');gpu_image=Image.open(gpu_path).convert('RGBA')
    assert cpu_image.size==gpu_image.size
    a=np.asarray(cpu_image);b=np.asarray(gpu_image);diff=np.abs(a.astype(np.int16)-b.astype(np.int16))
    alpha=diff[:,:,3];rgbmax=diff[:,:,:3].max(axis=2);visible=(a[:,:,3]>0)|(b[:,:,3]>0);hidden=~visible
    alpha_a=a[:,:,3].astype(np.float64)/255.;alpha_b=b[:,:,3].astype(np.float64)/255.
    associated_a=linear(a[:,:,:3].astype(np.float64)/255.)*alpha_a[:,:,None]
    associated_b=linear(b[:,:,:3].astype(np.float64)/255.)*alpha_b[:,:,None]
    associated_diff=np.abs(associated_a-associated_b).max(axis=2)
    record={'cpu':str(cpu_path.relative_to(ROOT)),'gpu':str(gpu_path.relative_to(ROOT)),'cpu_png_sha256':sha(cpu_path),'gpu_png_sha256':sha(gpu_path),'size':cpu_image.size,'pixel_count':int(alpha.size),'different_alpha_pixels':int(np.count_nonzero(alpha)),'signed_alpha_mean_gpu_minus_cpu_bytes':float((b[:,:,3].astype(np.float64)-a[:,:,3]).mean()),'signed_associated_linear_rgb_mean_gpu_minus_cpu':(associated_b-associated_a).mean(axis=(0,1)).tolist(),'signed_straight_rgb_mean_gpu_minus_cpu_visible_bytes':(b[:,:,:3].astype(np.float64)-a[:,:,:3])[visible].mean(axis=0).tolist(),'alpha_byte_error':distribution(alpha),'visible_pixel_count':int(np.count_nonzero(visible)),'different_visible_straight_rgb_pixels':int(np.count_nonzero(rgbmax[visible])),'visible_straight_rgb_max_channel_byte_error':distribution(rgbmax[visible]),'both_zero_alpha_pixel_count':int(np.count_nonzero(hidden)),'different_hidden_rgb_pixels':int(np.count_nonzero(rgbmax[hidden])),'associated_linear_rgb_max_channel_absolute_error':distribution(associated_diff),'associated_linear_rgb_rmse_all_channels':float(np.sqrt(np.mean((associated_a-associated_b)**2))),'interpretation':'Metrics are error descriptions, NOTperceptual/JND acceptance thresholds. StraightRGBerrors canbe large atlowalpha. AssociatedlinearRGBweightscolorbyalpha andzeroes hiddenRGB; inspectseparatealpha andnativeRGBAimages. Noartworkbackgroundcomposite.'}
    Image.fromarray(a[:,:,3]).save(out/'cpu-alpha8.png');Image.fromarray(b[:,:,3]).save(out/'gpu-alpha8.png');Image.fromarray(alpha.astype(np.uint8)).save(out/'absolute-alpha-error8.png');np.save(out/'associated-linear-rgb-max-error.npy',associated_diff)
    alpha_display=Image.fromarray(np.clip(alpha*4,0,255).astype(np.uint8));associated_display=Image.fromarray(np.clip(associated_diff*4*255,0,255).astype(np.uint8))
    alpha_display.save(out/'alpha-error-display-gain4.png');associated_display.save(out/'associated-linear-error-display-gain4.png')
    width,height=cpu_image.size;panel_w=450;panel_h=round(height*panel_w/width);font=ImageFont.load_default(size=14);figure=Image.new('RGB',(940,panel_h+95),'white');draw=ImageDraw.Draw(figure)
    draw.text((10,5),'Natural-artwork scalar error diagnostics; native RGBA files remain unchanged',font=font,fill='black')
    draw.text((10,25),f'Original{width}x{height}; overview nearest-neighbor resize to{panel_w}x{panel_h}. Noacceptance threshold.',font=font,fill='black')
    for x,label,image in [(10,'Absolute alpha byte error; gain4, clipped0..255',alpha_display),(480,'Associated-linear RGBmax error; gain4, clipped0..1',associated_display)]:
        draw.text((x,50),label,font=font,fill='black');figure.paste(image.resize((panel_w,panel_h),Image.Resampling.NEAREST),(x,80))
    figure.save(out/'error-overview.png')
    crops=[(360,210,500,350),(520,200,720,400)] if width==900 else ([(400,400,600,600),(30,30,230,230)] if width==1024 else [(max(0,width//2-100),max(0,height//2-100),min(width,width//2+100),min(height,height//2+100))])
    for i,crop in enumerate(crops):cpu_image.crop(crop).save(out/f'cpu-crop{i}-100percent.png');gpu_image.crop(crop).save(out/f'gpu-crop{i}-100percent.png')
    record['crops_original_xyxy']=crops;record['crop_rule']='ExactnativeRGBApixelsat1:1,noresize/noflatten';record['scalar_display_rule']='Alphaerrorgain4 (0..63.75bytes maps0..255),associatedlinearerrorgain4 (0..0.25maps0..255),clipdisplayonly; exactscalarplanes/arrayretained';record['script_sha256']=sha(Path(__file__))
    (out/'metrics-and-provenance.json').write_text(json.dumps(record,indent=2)+'\n')
    return record


if __name__=='__main__':
    cpu,gpu,output=sys.argv[1:];out=Path(output)
    if out.exists():raise SystemExit('freshdiagnosticoutputrequired')
    out.mkdir(parents=True)
    print(json.dumps(analyze(cpu,gpu,out),indent=2))
