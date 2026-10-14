/* Synthetic JPEG XL regression fixtures. SPDX-License-Identifier: CC0-1.0
 * Build/run: cc generate.c -o /tmp/toniator-jxl-fixtures $(pkg-config --cflags --libs libjxl lcms2)
 * Run from this directory. Native encoder is fixture tooling, never an application dependency.
 */
#include <jxl/encode.h>
#include <lcms2.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define CHECK(x) do { if ((x) != JXL_ENC_SUCCESS) abort(); } while (0)
static void fixture(const char *name, int mode) {
    JxlEncoder *enc = JxlEncoderCreate(NULL);
    JxlBasicInfo info;
    JxlEncoderInitBasicInfo(&info);
    info.xsize = 3; info.ysize = 2; info.bits_per_sample = mode == 4 ? 16 : 8;
    info.num_color_channels = mode == 3 ? 1 : 3;
    info.num_extra_channels = mode == 3 ? 0 : 1;
    info.alpha_bits = mode == 3 ? 0 : info.bits_per_sample;
    info.alpha_premultiplied = mode == 1;
    info.uses_original_profile = JXL_TRUE;
    info.orientation = mode == 5 ? JXL_ORIENT_ROTATE_90_CW : JXL_ORIENT_IDENTITY;
    info.have_animation = mode == 2 || mode == 7;
    info.animation.tps_numerator = 4; info.animation.tps_denominator = 1;
    info.animation.num_loops = 0;
    CHECK(JxlEncoderSetBasicInfo(enc, &info));
    if(mode==1) { JxlExtraChannelInfo extra; JxlEncoderInitExtraChannelInfo(JXL_CHANNEL_ALPHA,&extra); extra.bits_per_sample=8; extra.alpha_premultiplied=JXL_TRUE; CHECK(JxlEncoderSetExtraChannelInfo(enc,0,&extra)); }
    if (mode == 6) {
        cmsCIExyY white; cmsWhitePointFromTemp(&white, 6504);
        cmsCIExyYTRIPLE primaries = {{0.64,0.33,1},{0.30,0.60,1},{0.15,0.06,1}};
        cmsToneCurve *curves[3]; for (int i=0;i<3;i++) curves[i]=cmsBuildGamma(NULL,1.0);
        cmsHPROFILE profile=cmsCreateRGBProfile(&white,&primaries,curves);
        cmsUInt32Number size=0; cmsSaveProfileToMem(profile,NULL,&size);
        void *bytes=malloc(size); cmsSaveProfileToMem(profile,bytes,&size);
        CHECK(JxlEncoderSetICCProfile(enc,bytes,size)); free(bytes); cmsCloseProfile(profile);
        for (int i=0;i<3;i++) cmsFreeToneCurve(curves[i]);
    } else {
        JxlColorEncoding color; JxlColorEncodingSetToSRGB(&color,mode==3);
        CHECK(JxlEncoderSetColorEncoding(enc,&color));
    }
    CHECK(JxlEncoderUseContainer(enc,JXL_TRUE));
    JxlEncoderFrameSettings *settings=JxlEncoderFrameSettingsCreate(enc,NULL);
    CHECK(JxlEncoderSetFrameLossless(settings,JXL_TRUE));
    uint8_t pixels[24]={255,0,0,255, 0,255,0,128, 0,0,255,0, 64,128,192,255, 255,255,255,64, 0,0,0,255};
    uint16_t wide[24]; for(int i=0;i<24;i++) wide[i]=(uint16_t)pixels[i]*257;
    JxlPixelFormat format={mode==3?1:4,mode==4?JXL_TYPE_UINT16:JXL_TYPE_UINT8,JXL_NATIVE_ENDIAN,0};
    for(int frame=0;frame<((mode==2 || mode==7)?3:1);frame++) {
        if(mode==2) {
            JxlFrameHeader header; JxlEncoderInitFrameHeader(&header); header.duration=frame+1;
            CHECK(JxlEncoderSetFrameHeader(settings,&header));
            for(int i=0;i<6;i++) { memset(pixels+i*4,0,4); pixels[i*4+frame]=255; pixels[i*4+3]=frame==1?128:255; }
        }
        if(mode==7) {
            JxlFrameHeader header; JxlEncoderInitFrameHeader(&header);
            header.duration=frame==1?0:1;
            header.layer_info.save_as_reference=frame==0?1:2;
            if(frame>0) {
                header.layer_info.have_crop=JXL_TRUE; header.layer_info.crop_x0=frame;
                header.layer_info.xsize=1; header.layer_info.ysize=1;
                header.layer_info.blend_info.blendmode=JXL_BLEND_BLEND;
                header.layer_info.blend_info.source=frame==1?1:2;
                header.layer_info.blend_info.alpha=0;
            }
            CHECK(JxlEncoderSetFrameHeader(settings,&header));
            if(frame>0) CHECK(JxlEncoderSetExtraChannelBlendInfo(settings,0,&header.layer_info.blend_info));
            for(int i=0;i<6;i++) { memset(pixels+i*4,0,4); pixels[i*4+frame]=255; pixels[i*4+3]=frame==2?128:255; }
        }
        if(mode==6) for(int i=0;i<24;i++) pixels[i]=128;
        if(mode==1) for(int i=0;i<24;i++) pixels[i]=64;
        CHECK(JxlEncoderAddImageFrame(settings,&format,mode==4?(void*)wide:(void*)pixels,mode==3?6:mode==4?48:mode==7&&frame>0?4:24));
    }
    JxlEncoderCloseInput(enc);
    uint8_t *bytes=malloc(65536),*next=bytes; size_t left=65536;
    CHECK(JxlEncoderProcessOutput(enc,&next,&left));
    FILE *file=fopen(name,"wb"); if(!file) abort(); fwrite(bytes,1,65536-left,file); fclose(file);
    free(bytes); JxlEncoderDestroy(enc);
}
int main(void) {
    fixture("associated-alpha.jxl",1); fixture("vfr-loop-animation.jxl",2);
    fixture("gray.jxl",3); fixture("rgba16.jxl",4); fixture("oriented.jxl",5);
    fixture("linear-icc.jxl",6); fixture("reference-layers.jxl",7); return 0;
}
