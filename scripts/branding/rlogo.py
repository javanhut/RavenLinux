"""Minimal PNG read/write + trim/pad/resize/composite, no third-party deps."""
import zlib, struct, sys

def read_png(path):
    data=open(path,'rb').read()
    assert data[:8]==b'\x89PNG\r\n\x1a\n'
    pos=8; idat=b''; w=h=None
    while pos<len(data):
        ln=struct.unpack('>I',data[pos:pos+4])[0]; typ=data[pos+4:pos+8]; body=data[pos+8:pos+8+ln]; pos+=12+ln
        if typ==b'IHDR':
            w,h,bd,ct,_,_,il=struct.unpack('>IIBBBBB',body); assert bd==8 and il==0, (bd,il)
        elif typ==b'IDAT': idat+=body
        elif typ==b'IEND': break
    nch={6:4,2:3,0:1,4:2}[ct]
    raw=zlib.decompress(idat); stride=w*nch
    out=bytearray(w*h*4); prev=bytearray(stride); p=0
    for y in range(h):
        f=raw[p]; p+=1; cur=bytearray(raw[p:p+stride]); p+=stride
        for i in range(stride):
            a=cur[i-nch] if i>=nch else 0; b=prev[i]; c=prev[i-nch] if i>=nch else 0
            if f==1: cur[i]=(cur[i]+a)&255
            elif f==2: cur[i]=(cur[i]+b)&255
            elif f==3: cur[i]=(cur[i]+((a+b)>>1))&255
            elif f==4:
                pa=abs(b-c); pb=abs(a-c); pc=abs(a+b-2*c)
                pr=a if pa<=pb and pa<=pc else (b if pb<=pc else c)
                cur[i]=(cur[i]+pr)&255
        for x in range(w):
            o=(y*w+x)*4
            if nch==4: out[o:o+4]=cur[x*4:x*4+4]
            elif nch==3: out[o:o+3]=cur[x*3:x*3+3]; out[o+3]=255
            elif nch==1: out[o]=out[o+1]=out[o+2]=cur[x]; out[o+3]=255
            else: out[o]=out[o+1]=out[o+2]=cur[x*2]; out[o+3]=cur[x*2+1]
        prev=cur
    return w,h,out

def write_png(path,w,h,px,alpha=True):
    nch=4 if alpha else 3
    raw=bytearray()
    for y in range(h):
        raw.append(0)
        for x in range(w):
            o=(y*w+x)*4; raw+=px[o:o+nch]
    def chunk(t,b): return struct.pack('>I',len(b))+t+b+struct.pack('>I',zlib.crc32(t+b)&0xffffffff)
    ihdr=struct.pack('>IIBBBBB',w,h,8,6 if alpha else 2,0,0,0)
    open(path,'wb').write(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',ihdr)+chunk(b'IDAT',zlib.compress(bytes(raw),9))+chunk(b'IEND',b''))

def bbox(w,h,px,thr=8):
    minx,miny,maxx,maxy=w,h,-1,-1
    for y in range(h):
        for x in range(w):
            if px[(y*w+x)*4+3]>thr:
                if x<minx:minx=x
                if x>maxx:maxx=x
                if y<miny:miny=y
                if y>maxy:maxy=y
    return minx,miny,maxx+1,maxy+1

def crop(w,h,px,x0,y0,x1,y1):
    nw,nh=x1-x0,y1-y0; out=bytearray(nw*nh*4)
    for y in range(nh):
        out[y*nw*4:(y+1)*nw*4]=px[((y+y0)*w+x0)*4:((y+y0)*w+x1)*4]
    return nw,nh,out

def pad_square(w,h,px,margin=0.0):
    s=int(round(max(w,h)*(1+2*margin))); out=bytearray(s*s*4)
    ox,oy=(s-w)//2,(s-h)//2
    for y in range(h): out[((y+oy)*s+ox)*4:((y+oy)*s+ox+w)*4]=px[y*w*4:(y+1)*w*4]
    return s,s,out

def resize(w,h,px,nw,nh):
    """Area-averaging resample with premultiplied alpha (good for downscale, ok for mild upscale)."""
    out=bytearray(nw*nh*4)
    for ny in range(nh):
        sy0=ny*h/nh; sy1=(ny+1)*h/nh
        for nx in range(nw):
            sx0=nx*w/nw; sx1=(nx+1)*w/nw
            r=g=b=a=0.0; tot=0.0
            y=int(sy0)
            while y<sy1 and y<h:
                wy=min(sy1,y+1)-max(sy0,y)
                x=int(sx0)
                while x<sx1 and x<w:
                    wx=min(sx1,x+1)-max(sx0,x); wt=wx*wy
                    o=(y*w+x)*4; pa=px[o+3]/255.0
                    r+=px[o]*pa*wt; g+=px[o+1]*pa*wt; b+=px[o+2]*pa*wt; a+=pa*wt; tot+=wt
                    x+=1
                y+=1
            o=(ny*nw+nx)*4
            if a>0:
                out[o]=min(255,int(r/a+0.5)); out[o+1]=min(255,int(g/a+0.5)); out[o+2]=min(255,int(b/a+0.5)); out[o+3]=min(255,int(a/tot*255+0.5))
    return nw,nh,out

def composite(w,h,px,bg):
    out=bytearray(px)
    for i in range(w*h):
        o=i*4; a=px[o+3]/255.0
        for c in range(3): out[o+c]=int(px[o+c]*a+bg[c]*(1-a)+0.5)
        out[o+3]=255
    return w,h,out

def recolor(w,h,px,fn):
    """fn(r,g,b,a)->(r,g,b) applied to every pixel with alpha>0."""
    out=bytearray(px)
    for i in range(w*h):
        o=i*4
        if px[o+3]:
            r,g,b=fn(px[o],px[o+1],px[o+2],px[o+3]); out[o],out[o+1],out[o+2]=r,g,b
    return w,h,out
