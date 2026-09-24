/* Experimental read-only Classic Mac OS catalog viewer. No POSIX paths,
   no deletion, no assumptions about modern macOS cache locations. */
#include <Quickdraw.h>
#include <Windows.h>
#include <Fonts.h>
#include <Menus.h>
#include <Events.h>
#include <Files.h>
#include <Memory.h>
#include <stdio.h>
#include <string.h>
#include <math.h>

#define fsRtDirID 2L
#define MAX_ITEMS 256
#define MAX_DEPTH 128
#define PI 3.141592653589793

typedef struct { Str255 name; long dir; double bytes; int directory; } Item;
typedef struct { long dir; short index; int owner; } Frame;
static Item items[MAX_ITEMS];
static Frame stack[MAX_DEPTH];
static WindowPtr window;
static short volume;
static long current = fsRtDirID, parents[MAX_DEPTH];
static int itemCount, depth, parentCount, running, errors;
static unsigned long visited;

static void label(short x, short y, const char *text) {
    Str255 p; size_t n = strlen(text); if(n > 255) n = 255;
    p[0] = n; memcpy(p+1, text, n); MoveTo(x,y); DrawString(p);
}
static void begin(long directory) {
    current = directory; itemCount = 0; depth = 1;
    stack[0].dir = directory; stack[0].index = 0; stack[0].owner = -1;
    visited = 0; errors = 0; running = 1;
}
/* Bounded work per event-loop turn keeps redraw, navigation and Stop responsive. */
static void step(void) {
    int budget = 48;
    while(running && depth && budget--) {
        Frame *f = &stack[depth-1];
        CInfoPBRec pb; Str255 name; OSErr err; int owner;
        memset(&pb,0,sizeof(pb));
        pb.dirInfo.ioVRefNum = volume; pb.dirInfo.ioDrDirID = f->dir;
        pb.dirInfo.ioFDirIndex = ++f->index; pb.dirInfo.ioNamePtr = name;
        err = PBGetCatInfoSync(&pb);
        if(err != noErr) { if(err != fnfErr) errors++; depth--; continue; }
        visited++; owner = f->owner;
        if(depth == 1) {
            if(itemCount == MAX_ITEMS) { errors++; running = 0; break; }
            owner = itemCount++;
            memcpy(items[owner].name,name,name[0]+1);
            items[owner].bytes = 0;
            items[owner].directory = (pb.hFileInfo.ioFlAttrib & 16) != 0;
            items[owner].dir = pb.dirInfo.ioDrDirID;
        }
        if(pb.hFileInfo.ioFlAttrib & 16) {
            if(depth == MAX_DEPTH) { errors++; continue; }
            stack[depth].dir = pb.dirInfo.ioDrDirID;
            stack[depth].index = 0; stack[depth].owner = owner; depth++;
        } else {
            items[owner].bytes += (double)(unsigned long)pb.hFileInfo.ioFlLgLen
                               + (double)(unsigned long)pb.hFileInfo.ioFlRLgLen;
        }
    }
    if(!depth) running = 0;
}
static void draw(void) {
    Rect r; int i; double maximum = 1, total = 0; char text[200];
    SetPort(window); SetRect(&r,0,0,620,430); EraseRect(&r);
    label(15,23,"Yeti3 Cleaner Classic - experimental read-only preview");
    label(15,46,"Click a ray: open folder. U: up. S: stop. R: rescan. Q: quit.");
    for(i=0;i<itemCount;i++) { total+=items[i].bytes; if(items[i].bytes>maximum) maximum=items[i].bytes; }
    for(i=0;i<itemCount;i++) {
        int radius = 45 + (int)(130 * items[i].bytes/maximum);
        SetRect(&r,205-radius,235-radius,205+radius,235+radius);
        PenPat((i%2) ? &qd.gray : &qd.black);
        FrameArc(&r,(short)(360.0*i/itemCount),(short)(360.0/itemCount+1));
        MoveTo(205,235);
        LineTo(205+(short)(radius*sin(2*PI*i/itemCount)),235-(short)(radius*cos(2*PI*i/itemCount)));
    }
    PenNormal();
    sprintf(text,"%.1f MiB found",total/1048576.0); label(395,85,text);
    sprintf(text,"%lu objects read",visited); label(395,108,text);
    label(395,131,running ? "Reading... (sizes grow)" : "Stopped / finished");
    sprintf(text,"%d skipped / limits",errors); label(395,154,text);
    for(i=0;i<itemCount && i<10;i++) {
        MoveTo(395,185+i*20); DrawString(items[i].name);
    }
    label(15,421,"Includes data and resource forks. Partial results are not free-space totals.");
}
int main(void) {
    Rect r; EventRecord event; int quit = 0; long lastDraw = 0;
    InitGraf(&qd.thePort); InitFonts(); InitWindows(); InitMenus(); InitCursor();
    SetRect(&r,25,45,645,475);
    window = NewWindow(NULL,&r,(ConstStringPtr)"\pYeti3 Classic preview",true,0,(WindowPtr)-1,true,0);
    if(!window) return 1;
    SetPort(window); TextFont(0); TextSize(12);
    GetVol(NULL,&volume); begin(fsRtDirID);
    while(!quit) {
        if(WaitNextEvent(everyEvent,&event,running ? 1 : 6,NULL)) {
            if(event.what == updateEvt) { BeginUpdate(window); draw(); EndUpdate(window); }
            else if(event.what == keyDown || event.what == autoKey) {
                char key = event.message & charCodeMask;
                if(key=='q' || key=='Q') quit=1;
                if(key=='s' || key=='S' || key==27) running=0;
                if(key=='r' || key=='R') begin(current);
                if((key=='u' || key=='U') && parentCount) begin(parents[--parentCount]);
                draw();
            } else if(event.what == mouseDown) {
                WindowPtr clicked; short part = FindWindow(event.where,&clicked);
                if(part==inGoAway && TrackGoAway(window,event.where)) quit=1;
                else if(part==inDrag) { r=qd.screenBits.bounds; DragWindow(window,event.where,&r); }
                else if(part==inContent && itemCount) {
                    Point p=event.where; double angle; int index;
                    SetPort(window); GlobalToLocal(&p);
                    angle=atan2(p.h-205.0,235.0-p.v); if(angle<0) angle+=2*PI;
                    index=(int)(angle/(2*PI)*itemCount);
                    if(p.h<390 && index<itemCount && items[index].directory && parentCount<MAX_DEPTH) {
                        long next=items[index].dir; parents[parentCount++]=current; begin(next); draw();
                    }
                }
            }
        }
        if(running) step();
        if(TickCount()-lastDraw>=10) { draw(); lastDraw=TickCount(); }
        SystemTask();
    }
    DisposeWindow(window); return 0;
}
