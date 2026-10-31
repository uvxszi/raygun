// Raygun Nuclear plugin converter. Built by packages/nuclear-compat (pnpm build); do not edit.
"use strict";(()=>{var na=Object.create;var uo=Object.defineProperty;var ra=Object.getOwnPropertyDescriptor;var sa=Object.getOwnPropertyNames;var oa=Object.getPrototypeOf,ia=Object.prototype.hasOwnProperty;var en=(e,n)=>()=>(n||e((n={exports:{}}).exports,n),n.exports);var aa=(e,n,r,o)=>{if(n&&typeof n=="object"||typeof n=="function")for(let i of sa(n))!ia.call(e,i)&&i!==r&&uo(e,i,{get:()=>n[i],enumerable:!(o=ra(n,i))||o.enumerable});return e};var Xr=(e,n,r)=>(r=e!=null?na(oa(e)):{},aa(n||!e||!e.__esModule?uo(r,"default",{value:e,enumerable:!0}):r,e));var fn=en(de=>{"use strict";var Qa=de&&de.__extends||(function(){var e=function(n,r){return e=Object.setPrototypeOf||{__proto__:[]}instanceof Array&&function(o,i){o.__proto__=i}||function(o,i){for(var c in i)i.hasOwnProperty(c)&&(o[c]=i[c])},e(n,r)};return function(n,r){e(n,r);function o(){this.constructor=n}n.prototype=r===null?Object.create(r):(o.prototype=r.prototype,new o)}})();Object.defineProperty(de,"__esModule",{value:!0});de.DetailContext=de.NoopContext=de.VError=void 0;var Fo=(function(e){Qa(n,e);function n(r,o){var i=e.call(this,o)||this;return i.path=r,Object.setPrototypeOf(i,n.prototype),i}return n})(Error);de.VError=Fo;var Ya=(function(){function e(){}return e.prototype.fail=function(n,r,o){return!1},e.prototype.unionResolver=function(){return this},e.prototype.createContext=function(){return this},e.prototype.resolveUnion=function(n){},e})();de.NoopContext=Ya;var Mo=(function(){function e(){this._propNames=[""],this._messages=[null],this._score=0}return e.prototype.fail=function(n,r,o){return this._propNames.push(n),this._messages.push(r),this._score+=o,!1},e.prototype.unionResolver=function(){return new Za},e.prototype.resolveUnion=function(n){for(var r,o,i=n,c=null,p=0,h=i.contexts;p<h.length;p++){var x=h[p];(!c||x._score>=c._score)&&(c=x)}c&&c._score>0&&((r=this._propNames).push.apply(r,c._propNames),(o=this._messages).push.apply(o,c._messages))},e.prototype.getError=function(n){for(var r=[],o=this._propNames.length-1;o>=0;o--){var i=this._propNames[o];n+=typeof i=="number"?"["+i+"]":i?"."+i:"";var c=this._messages[o];c&&r.push(n+" "+c)}return new Fo(n,r.join("; "))},e.prototype.getErrorDetail=function(n){for(var r=[],o=this._propNames.length-1;o>=0;o--){var i=this._propNames[o];n+=typeof i=="number"?"["+i+"]":i?"."+i:"";var c=this._messages[o];c&&r.push({path:n,message:c})}for(var p=null,o=r.length-1;o>=0;o--)p&&(r[o].nested=[p]),p=r[o];return p},e})();de.DetailContext=Mo;var Za=(function(){function e(){this.contexts=[]}return e.prototype.createContext=function(){var n=new Mo;return this.contexts.push(n),n},e})()});var Is=en(T=>{"use strict";var se=T&&T.__extends||(function(){var e=function(n,r){return e=Object.setPrototypeOf||{__proto__:[]}instanceof Array&&function(o,i){o.__proto__=i}||function(o,i){for(var c in i)i.hasOwnProperty(c)&&(o[c]=i[c])},e(n,r)};return function(n,r){e(n,r);function o(){this.constructor=n}n.prototype=r===null?Object.create(r):(o.prototype=r.prototype,new o)}})();Object.defineProperty(T,"__esModule",{value:!0});T.basicTypes=T.BasicType=T.TParamList=T.TParam=T.param=T.TFunc=T.func=T.TProp=T.TOptional=T.opt=T.TIface=T.iface=T.TEnumLiteral=T.enumlit=T.TEnumType=T.enumtype=T.TIntersection=T.intersection=T.TUnion=T.union=T.TTuple=T.tuple=T.TArray=T.array=T.TLiteral=T.lit=T.TName=T.name=T.TType=void 0;var $o=fn(),te=(function(){function e(){}return e})();T.TType=te;function Ee(e){return typeof e=="string"?Uo(e):e}function xs(e,n){var r=e[n];if(!r)throw new Error("Unknown type "+n);return r}function Uo(e){return new ys(e)}T.name=Uo;var ys=(function(e){se(n,e);function n(r){var o=e.call(this)||this;return o.name=r,o._failMsg="is not a "+r,o}return n.prototype.getChecker=function(r,o,i){var c=this,p=xs(r,this.name),h=p.getChecker(r,o,i);return p instanceof K||p instanceof n?h:function(x,k){return h(x,k)?!0:k.fail(null,c._failMsg,0)}},n})(te);T.TName=ys;function ec(e){return new _s(e)}T.lit=ec;var _s=(function(e){se(n,e);function n(r){var o=e.call(this)||this;return o.value=r,o.name=JSON.stringify(r),o._failMsg="is not "+o.name,o}return n.prototype.getChecker=function(r,o){var i=this;return function(c,p){return c===i.value?!0:p.fail(null,i._failMsg,-1)}},n})(te);T.TLiteral=_s;function tc(e){return new Ho(Ee(e))}T.array=tc;var Ho=(function(e){se(n,e);function n(r){var o=e.call(this)||this;return o.ttype=r,o}return n.prototype.getChecker=function(r,o){var i=this.ttype.getChecker(r,o);return function(c,p){if(!Array.isArray(c))return p.fail(null,"is not an array",0);for(var h=0;h<c.length;h++){var x=i(c[h],p);if(!x)return p.fail(h,null,1)}return!0}},n})(te);T.TArray=Ho;function nc(){for(var e=[],n=0;n<arguments.length;n++)e[n]=arguments[n];return new Vo(e.map(function(r){return Ee(r)}))}T.tuple=nc;var Vo=(function(e){se(n,e);function n(r){var o=e.call(this)||this;return o.ttypes=r,o}return n.prototype.getChecker=function(r,o){var i=this.ttypes.map(function(p){return p.getChecker(r,o)}),c=function(p,h){if(!Array.isArray(p))return h.fail(null,"is not an array",0);for(var x=0;x<i.length;x++){var k=i[x](p[x],h);if(!k)return h.fail(x,null,1)}return!0};return o?function(p,h){return c(p,h)?p.length<=i.length?!0:h.fail(i.length,"is extraneous",2):!1}:c},n})(te);T.TTuple=Vo;function rc(){for(var e=[],n=0;n<arguments.length;n++)e[n]=arguments[n];return new Wo(e.map(function(r){return Ee(r)}))}T.union=rc;var Wo=(function(e){se(n,e);function n(r){var o=e.call(this)||this;o.ttypes=r;var i=r.map(function(p){return p instanceof ys||p instanceof _s?p.name:null}).filter(function(p){return p}),c=r.length-i.length;return i.length?(c>0&&i.push(c+" more"),o._failMsg="is none of "+i.join(", ")):o._failMsg="is none of "+c+" types",o}return n.prototype.getChecker=function(r,o){var i=this,c=this.ttypes.map(function(p){return p.getChecker(r,o)});return function(p,h){for(var x=h.unionResolver(),k=0;k<c.length;k++){var y=c[k](p,x.createContext());if(y)return!0}return h.resolveUnion(x),h.fail(null,i._failMsg,0)}},n})(te);T.TUnion=Wo;function sc(){for(var e=[],n=0;n<arguments.length;n++)e[n]=arguments[n];return new zo(e.map(function(r){return Ee(r)}))}T.intersection=sc;var zo=(function(e){se(n,e);function n(r){var o=e.call(this)||this;return o.ttypes=r,o}return n.prototype.getChecker=function(r,o){var i=new Set,c=this.ttypes.map(function(p){return p.getChecker(r,o,i)});return function(p,h){var x=c.every(function(k){return k(p,h)});return x?!0:h.fail(null,null,0)}},n})(te);T.TIntersection=zo;function oc(e){return new bs(e)}T.enumtype=oc;var bs=(function(e){se(n,e);function n(r){var o=e.call(this)||this;return o.members=r,o.validValues=new Set,o._failMsg="is not a valid enum value",o.validValues=new Set(Object.keys(r).map(function(i){return r[i]})),o}return n.prototype.getChecker=function(r,o){var i=this;return function(c,p){return i.validValues.has(c)?!0:p.fail(null,i._failMsg,0)}},n})(te);T.TEnumType=bs;function ic(e,n){return new Go(e,n)}T.enumlit=ic;var Go=(function(e){se(n,e);function n(r,o){var i=e.call(this)||this;return i.enumName=r,i.prop=o,i._failMsg="is not "+r+"."+o,i}return n.prototype.getChecker=function(r,o){var i=this,c=xs(r,this.enumName);if(!(c instanceof bs))throw new Error("Type "+this.enumName+" used in enumlit is not an enum type");var p=c.members[this.prop];if(!c.members.hasOwnProperty(this.prop))throw new Error("Unknown value "+this.enumName+"."+this.prop+" used in enumlit");return function(h,x){return h===p?!0:x.fail(null,i._failMsg,-1)}},n})(te);T.TEnumLiteral=Go;function ac(e){return Object.keys(e).map(function(n){return cc(n,e[n])})}function cc(e,n){return n instanceof Ts?new gs(e,n.ttype,!0):new gs(e,Ee(n),!1)}function lc(e,n){return new Xo(e,ac(n))}T.iface=lc;var Xo=(function(e){se(n,e);function n(r,o){var i=e.call(this)||this;return i.bases=r,i.props=o,i.propSet=new Set(o.map(function(c){return c.name})),i}return n.prototype.getChecker=function(r,o,i){var c=this,p=this.bases.map(function(I){return xs(r,I).getChecker(r,o)}),h=this.props.map(function(I){return I.ttype.getChecker(r,o)}),x=new $o.NoopContext,k=this.props.map(function(I,A){return!I.isOpt&&!h[A](void 0,x)}),y=function(I,A){if(typeof I!="object"||I===null)return A.fail(null,"is not an object",0);for(var O=0;O<p.length;O++)if(!p[O](I,A))return!1;for(var O=0;O<h.length;O++){var E=c.props[O].name,$=I[E];if($===void 0){if(k[O])return A.fail(E,"is missing",1)}else{var Q=h[O]($,A);if(!Q)return A.fail(E,null,1)}}return!0};if(!o)return y;var w=this.propSet;return i&&(this.propSet.forEach(function(I){return i.add(I)}),w=i),function(I,A){if(!y(I,A))return!1;for(var O in I)if(!w.has(O))return A.fail(O,"is extraneous",2);return!0}},n})(te);T.TIface=Xo;function uc(e){return new Ts(Ee(e))}T.opt=uc;var Ts=(function(e){se(n,e);function n(r){var o=e.call(this)||this;return o.ttype=r,o}return n.prototype.getChecker=function(r,o){var i=this.ttype.getChecker(r,o);return function(c,p){return c===void 0||i(c,p)}},n})(te);T.TOptional=Ts;var gs=(function(){function e(n,r,o){this.name=n,this.ttype=r,this.isOpt=o}return e})();T.TProp=gs;function pc(e){for(var n=[],r=1;r<arguments.length;r++)n[r-1]=arguments[r];return new Jo(new Qo(n),Ee(e))}T.func=pc;var Jo=(function(e){se(n,e);function n(r,o){var i=e.call(this)||this;return i.paramList=r,i.result=o,i}return n.prototype.getChecker=function(r,o){return function(i,c){return typeof i=="function"?!0:c.fail(null,"is not a function",0)}},n})(te);T.TFunc=Jo;function fc(e,n,r){return new Ko(e,Ee(n),!!r)}T.param=fc;var Ko=(function(){function e(n,r,o){this.name=n,this.ttype=r,this.isOpt=o}return e})();T.TParam=Ko;var Qo=(function(e){se(n,e);function n(r){var o=e.call(this)||this;return o.params=r,o}return n.prototype.getChecker=function(r,o){var i=this,c=this.params.map(function(k){return k.ttype.getChecker(r,o)}),p=new $o.NoopContext,h=this.params.map(function(k,y){return!k.isOpt&&!c[y](void 0,p)}),x=function(k,y){if(!Array.isArray(k))return y.fail(null,"is not an array",0);for(var w=0;w<c.length;w++){var I=i.params[w];if(k[w]===void 0){if(h[w])return y.fail(I.name,"is missing",1)}else{var A=c[w](k[w],y);if(!A)return y.fail(I.name,null,1)}}return!0};return o?function(k,y){return x(k,y)?k.length<=c.length?!0:y.fail(c.length,"is extraneous",2):!1}:x},n})(te);T.TParamList=Qo;var K=(function(e){se(n,e);function n(r,o){var i=e.call(this)||this;return i.validator=r,i.message=o,i}return n.prototype.getChecker=function(r,o){var i=this;return function(c,p){return i.validator(c)?!0:p.fail(null,i.message,0)}},n})(te);T.BasicType=K;T.basicTypes={any:new K(function(e){return!0},"is invalid"),number:new K(function(e){return typeof e=="number"},"is not a number"),object:new K(function(e){return typeof e=="object"&&e},"is not an object"),boolean:new K(function(e){return typeof e=="boolean"},"is not a boolean"),string:new K(function(e){return typeof e=="string"},"is not a string"),symbol:new K(function(e){return typeof e=="symbol"},"is not a symbol"),void:new K(function(e){return e==null},"is not void"),undefined:new K(function(e){return e===void 0},"is not undefined"),null:new K(function(e){return e===null},"is not null"),never:new K(function(e){return!1},"is unexpected"),Date:new K(Bo("[object Date]"),"is not a Date"),RegExp:new K(Bo("[object RegExp]"),"is not a RegExp")};var hc=Object.prototype.toString;function Bo(e){return function(n){return typeof n=="object"&&n&&hc.call(n)===e}}typeof Buffer<"u"&&(T.basicTypes.Buffer=new K(function(e){return Buffer.isBuffer(e)},"is not a Buffer"));var mc=function(e){T.basicTypes[e.name]=new K(function(n){return n instanceof e},"is not a "+e.name)};for(hn=0,ks=[Int8Array,Uint8Array,Uint8ClampedArray,Int16Array,Uint16Array,Int32Array,Uint32Array,Float32Array,Float64Array,ArrayBuffer];hn<ks.length;hn++)qo=ks[hn],mc(qo);var qo,hn,ks});var ws=en(D=>{"use strict";var dc=D&&D.__spreadArrays||function(){for(var e=0,n=0,r=arguments.length;n<r;n++)e+=arguments[n].length;for(var o=Array(e),i=0,n=0;n<r;n++)for(var c=arguments[n],p=0,h=c.length;p<h;p++,i++)o[i]=c[p];return o};Object.defineProperty(D,"__esModule",{value:!0});D.Checker=D.createCheckers=void 0;var wt=Is(),ot=fn(),q=Is();Object.defineProperty(D,"TArray",{enumerable:!0,get:function(){return q.TArray}});Object.defineProperty(D,"TEnumType",{enumerable:!0,get:function(){return q.TEnumType}});Object.defineProperty(D,"TEnumLiteral",{enumerable:!0,get:function(){return q.TEnumLiteral}});Object.defineProperty(D,"TFunc",{enumerable:!0,get:function(){return q.TFunc}});Object.defineProperty(D,"TIface",{enumerable:!0,get:function(){return q.TIface}});Object.defineProperty(D,"TLiteral",{enumerable:!0,get:function(){return q.TLiteral}});Object.defineProperty(D,"TName",{enumerable:!0,get:function(){return q.TName}});Object.defineProperty(D,"TOptional",{enumerable:!0,get:function(){return q.TOptional}});Object.defineProperty(D,"TParam",{enumerable:!0,get:function(){return q.TParam}});Object.defineProperty(D,"TParamList",{enumerable:!0,get:function(){return q.TParamList}});Object.defineProperty(D,"TProp",{enumerable:!0,get:function(){return q.TProp}});Object.defineProperty(D,"TTuple",{enumerable:!0,get:function(){return q.TTuple}});Object.defineProperty(D,"TType",{enumerable:!0,get:function(){return q.TType}});Object.defineProperty(D,"TUnion",{enumerable:!0,get:function(){return q.TUnion}});Object.defineProperty(D,"TIntersection",{enumerable:!0,get:function(){return q.TIntersection}});Object.defineProperty(D,"array",{enumerable:!0,get:function(){return q.array}});Object.defineProperty(D,"enumlit",{enumerable:!0,get:function(){return q.enumlit}});Object.defineProperty(D,"enumtype",{enumerable:!0,get:function(){return q.enumtype}});Object.defineProperty(D,"func",{enumerable:!0,get:function(){return q.func}});Object.defineProperty(D,"iface",{enumerable:!0,get:function(){return q.iface}});Object.defineProperty(D,"lit",{enumerable:!0,get:function(){return q.lit}});Object.defineProperty(D,"name",{enumerable:!0,get:function(){return q.name}});Object.defineProperty(D,"opt",{enumerable:!0,get:function(){return q.opt}});Object.defineProperty(D,"param",{enumerable:!0,get:function(){return q.param}});Object.defineProperty(D,"tuple",{enumerable:!0,get:function(){return q.tuple}});Object.defineProperty(D,"union",{enumerable:!0,get:function(){return q.union}});Object.defineProperty(D,"intersection",{enumerable:!0,get:function(){return q.intersection}});Object.defineProperty(D,"BasicType",{enumerable:!0,get:function(){return q.BasicType}});var kc=fn();Object.defineProperty(D,"VError",{enumerable:!0,get:function(){return kc.VError}});function gc(){for(var e=[],n=0;n<arguments.length;n++)e[n]=arguments[n];for(var r=Object.assign.apply(Object,dc([{},wt.basicTypes],e)),o={},i=0,c=e;i<c.length;i++)for(var p=c[i],h=0,x=Object.keys(p);h<x.length;h++){var k=x[h];o[k]=new Yo(r,p[k])}return o}D.createCheckers=gc;var Yo=(function(){function e(n,r,o){if(o===void 0&&(o="value"),this.suite=n,this.ttype=r,this._path=o,this.props=new Map,r instanceof wt.TIface)for(var i=0,c=r.props;i<c.length;i++){var p=c[i];this.props.set(p.name,p.ttype)}this.checkerPlain=this.ttype.getChecker(n,!1),this.checkerStrict=this.ttype.getChecker(n,!0)}return e.prototype.setReportedPath=function(n){this._path=n},e.prototype.check=function(n){return this._doCheck(this.checkerPlain,n)},e.prototype.test=function(n){return this.checkerPlain(n,new ot.NoopContext)},e.prototype.validate=function(n){return this._doValidate(this.checkerPlain,n)},e.prototype.strictCheck=function(n){return this._doCheck(this.checkerStrict,n)},e.prototype.strictTest=function(n){return this.checkerStrict(n,new ot.NoopContext)},e.prototype.strictValidate=function(n){return this._doValidate(this.checkerStrict,n)},e.prototype.getProp=function(n){var r=this.props.get(n);if(!r)throw new Error("Type has no property "+n);return new e(this.suite,r,this._path+"."+n)},e.prototype.methodArgs=function(n){var r=this._getMethod(n);return new e(this.suite,r.paramList)},e.prototype.methodResult=function(n){var r=this._getMethod(n);return new e(this.suite,r.result)},e.prototype.getArgs=function(){if(!(this.ttype instanceof wt.TFunc))throw new Error("getArgs() applied to non-function");return new e(this.suite,this.ttype.paramList)},e.prototype.getResult=function(){if(!(this.ttype instanceof wt.TFunc))throw new Error("getResult() applied to non-function");return new e(this.suite,this.ttype.result)},e.prototype.getType=function(){return this.ttype},e.prototype._doCheck=function(n,r){var o=new ot.NoopContext;if(!n(r,o)){var i=new ot.DetailContext;throw n(r,i),i.getError(this._path)}},e.prototype._doValidate=function(n,r){var o=new ot.NoopContext;if(n(r,o))return null;var i=new ot.DetailContext;return n(r,i),i.getErrorDetail(this._path)},e.prototype._getMethod=function(n){var r=this.props.get(n);if(!r)throw new Error("Type has no property "+n);if(!(r instanceof wt.TFunc))throw new Error("Property "+n+" is not a method");return r},e})();D.Checker=Yo});var j1=en(Zt=>{"use strict";Zt.__esModule=!0;Zt.LinesAndColumns=void 0;var Mn=`
`,D1="\r",O1=(function(){function e(n){this.string=n;for(var r=[0],o=0;o<n.length;)switch(n[o]){case Mn:o+=Mn.length,r.push(o);break;case D1:o+=D1.length,n[o]===Mn&&(o+=Mn.length),r.push(o);break;default:o++;break}this.offsets=r}return e.prototype.locationForIndex=function(n){if(n<0||n>this.string.length)return null;for(var r=0,o=this.offsets;o[r+1]<=n;)r++;var i=n-o[r];return{line:r,column:i}},e.prototype.indexForLocation=function(n){var r=n.line,o=n.column;return r<0||r>=this.offsets.length||o<0||o>this.lengthOfLine(r)?null:this.offsets[r]+o},e.prototype.lengthOfLine=function(n){var r=this.offsets[n],o=n===this.offsets.length-1?this.string.length:this.offsets[n+1];return o-r},e})();Zt.LinesAndColumns=O1;Zt.default=O1});var po=`"use strict";
var __nuclearShim = (() => {
  var __defProp = Object.defineProperty;
  var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
  var __getOwnPropNames = Object.getOwnPropertyNames;
  var __hasOwnProp = Object.prototype.hasOwnProperty;
  var __export = (target, all) => {
    for (var name in all)
      __defProp(target, name, { get: all[name], enumerable: true });
  };
  var __copyProps = (to, from, except, desc) => {
    if (from && typeof from === "object" || typeof from === "function") {
      for (let key of __getOwnPropNames(from))
        if (!__hasOwnProp.call(to, key) && key !== except)
          __defProp(to, key, { get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable });
    }
    return to;
  };
  var __toCommonJS = (mod) => __copyProps(__defProp({}, "__esModule", { value: true }), mod);

  // src/shim/index.ts
  var index_exports = {};
  __export(index_exports, {
    run: () => run
  });

  // ../../node_modules/.pnpm/fflate@0.8.3/node_modules/fflate/esm/browser.js
  var u8 = Uint8Array;
  var u16 = Uint16Array;
  var i32 = Int32Array;
  var fleb = new u8([
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    1,
    1,
    1,
    1,
    2,
    2,
    2,
    2,
    3,
    3,
    3,
    3,
    4,
    4,
    4,
    4,
    5,
    5,
    5,
    5,
    0,
    /* unused */
    0,
    0,
    /* impossible */
    0
  ]);
  var fdeb = new u8([
    0,
    0,
    0,
    0,
    1,
    1,
    2,
    2,
    3,
    3,
    4,
    4,
    5,
    5,
    6,
    6,
    7,
    7,
    8,
    8,
    9,
    9,
    10,
    10,
    11,
    11,
    12,
    12,
    13,
    13,
    /* unused */
    0,
    0
  ]);
  var clim = new u8([16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15]);
  var freb = function(eb, start) {
    var b = new u16(31);
    for (var i = 0; i < 31; ++i) {
      b[i] = start += 1 << eb[i - 1];
    }
    var r = new i32(b[30]);
    for (var i = 1; i < 30; ++i) {
      for (var j = b[i]; j < b[i + 1]; ++j) {
        r[j] = j - b[i] << 5 | i;
      }
    }
    return { b, r };
  };
  var _a = freb(fleb, 2);
  var fl = _a.b;
  var revfl = _a.r;
  fl[28] = 258, revfl[258] = 28;
  var _b = freb(fdeb, 0);
  var fd = _b.b;
  var revfd = _b.r;
  var rev = new u16(32768);
  for (i = 0; i < 32768; ++i) {
    x = (i & 43690) >> 1 | (i & 21845) << 1;
    x = (x & 52428) >> 2 | (x & 13107) << 2;
    x = (x & 61680) >> 4 | (x & 3855) << 4;
    rev[i] = ((x & 65280) >> 8 | (x & 255) << 8) >> 1;
  }
  var x;
  var i;
  var hMap = (function(cd, mb, r) {
    var s = cd.length;
    var i = 0;
    var l = new u16(mb);
    for (; i < s; ++i) {
      if (cd[i])
        ++l[cd[i] - 1];
    }
    var le = new u16(mb);
    for (i = 1; i < mb; ++i) {
      le[i] = le[i - 1] + l[i - 1] << 1;
    }
    var co;
    if (r) {
      co = new u16(1 << mb);
      var rvb = 15 - mb;
      for (i = 0; i < s; ++i) {
        if (cd[i]) {
          var sv = i << 4 | cd[i];
          var r_1 = mb - cd[i];
          var v = le[cd[i] - 1]++ << r_1;
          for (var m = v | (1 << r_1) - 1; v <= m; ++v) {
            co[rev[v] >> rvb] = sv;
          }
        }
      }
    } else {
      co = new u16(s);
      for (i = 0; i < s; ++i) {
        if (cd[i]) {
          co[i] = rev[le[cd[i] - 1]++] >> 15 - cd[i];
        }
      }
    }
    return co;
  });
  var flt = new u8(288);
  for (i = 0; i < 144; ++i)
    flt[i] = 8;
  var i;
  for (i = 144; i < 256; ++i)
    flt[i] = 9;
  var i;
  for (i = 256; i < 280; ++i)
    flt[i] = 7;
  var i;
  for (i = 280; i < 288; ++i)
    flt[i] = 8;
  var i;
  var fdt = new u8(32);
  for (i = 0; i < 32; ++i)
    fdt[i] = 5;
  var i;
  var flrm = /* @__PURE__ */ hMap(flt, 9, 1);
  var fdrm = /* @__PURE__ */ hMap(fdt, 5, 1);
  var max = function(a) {
    var m = a[0];
    for (var i = 1; i < a.length; ++i) {
      if (a[i] > m)
        m = a[i];
    }
    return m;
  };
  var bits = function(d, p, m) {
    var o = p / 8 | 0;
    return (d[o] | d[o + 1] << 8) >> (p & 7) & m;
  };
  var bits16 = function(d, p) {
    var o = p / 8 | 0;
    return (d[o] | d[o + 1] << 8 | d[o + 2] << 16) >> (p & 7);
  };
  var shft = function(p) {
    return (p + 7) / 8 | 0;
  };
  var slc = function(v, s, e) {
    if (s == null || s < 0)
      s = 0;
    if (e == null || e > v.length)
      e = v.length;
    return new u8(v.subarray(s, e));
  };
  var ec = [
    "unexpected EOF",
    "invalid block type",
    "invalid length/literal",
    "invalid distance",
    "stream finished",
    "no stream handler",
    ,
    // determined by compression function
    "no callback",
    "invalid UTF-8 data",
    "extra field too long",
    "date not in range 1980-2099",
    "filename too long",
    "stream finishing",
    "invalid zip data"
    // determined by unknown compression method
  ];
  var err = function(ind, msg, nt) {
    var e = new Error(msg || ec[ind]);
    e.code = ind;
    if (Error.captureStackTrace)
      Error.captureStackTrace(e, err);
    if (!nt)
      throw e;
    return e;
  };
  var inflt = function(dat, st, buf, dict) {
    var sl = dat.length, dl = dict ? dict.length : 0;
    if (!sl || st.f && !st.l)
      return buf || new u8(0);
    var noBuf = !buf;
    var resize = noBuf || st.i != 2;
    var noSt = st.i;
    if (noBuf)
      buf = new u8(sl * 3);
    var cbuf = function(l2) {
      var bl = buf.length;
      if (l2 > bl) {
        var nbuf = new u8(Math.max(bl * 2, l2));
        nbuf.set(buf);
        buf = nbuf;
      }
    };
    var final = st.f || 0, pos = st.p || 0, bt = st.b || 0, lm = st.l, dm = st.d, lbt = st.m, dbt = st.n;
    var tbts = sl * 8;
    do {
      if (!lm) {
        final = bits(dat, pos, 1);
        var type = bits(dat, pos + 1, 3);
        pos += 3;
        if (!type) {
          var s = shft(pos) + 4, l = dat[s - 4] | dat[s - 3] << 8, t = s + l;
          if (t > sl) {
            if (noSt)
              err(0);
            break;
          }
          if (resize)
            cbuf(bt + l);
          buf.set(dat.subarray(s, t), bt);
          st.b = bt += l, st.p = pos = t * 8, st.f = final;
          continue;
        } else if (type == 1)
          lm = flrm, dm = fdrm, lbt = 9, dbt = 5;
        else if (type == 2) {
          var hLit = bits(dat, pos, 31) + 257, hcLen = bits(dat, pos + 10, 15) + 4;
          var tl = hLit + bits(dat, pos + 5, 31) + 1;
          pos += 14;
          var ldt = new u8(tl);
          var clt = new u8(19);
          for (var i = 0; i < hcLen; ++i) {
            clt[clim[i]] = bits(dat, pos + i * 3, 7);
          }
          pos += hcLen * 3;
          var clb = max(clt), clbmsk = (1 << clb) - 1;
          var clm = hMap(clt, clb, 1);
          for (var i = 0; i < tl; ) {
            var r = clm[bits(dat, pos, clbmsk)];
            pos += r & 15;
            var s = r >> 4;
            if (s < 16) {
              ldt[i++] = s;
            } else {
              var c = 0, n = 0;
              if (s == 16)
                n = 3 + bits(dat, pos, 3), pos += 2, c = ldt[i - 1];
              else if (s == 17)
                n = 3 + bits(dat, pos, 7), pos += 3;
              else if (s == 18)
                n = 11 + bits(dat, pos, 127), pos += 7;
              while (n--)
                ldt[i++] = c;
            }
          }
          var lt = ldt.subarray(0, hLit), dt = ldt.subarray(hLit);
          lbt = max(lt);
          dbt = max(dt);
          lm = hMap(lt, lbt, 1);
          dm = hMap(dt, dbt, 1);
        } else
          err(1);
        if (pos > tbts) {
          if (noSt)
            err(0);
          break;
        }
      }
      if (resize)
        cbuf(bt + 131072);
      var lms = (1 << lbt) - 1, dms = (1 << dbt) - 1;
      var lpos = pos;
      for (; ; lpos = pos) {
        var c = lm[bits16(dat, pos) & lms], sym = c >> 4;
        pos += c & 15;
        if (pos > tbts) {
          if (noSt)
            err(0);
          break;
        }
        if (!c)
          err(2);
        if (sym < 256)
          buf[bt++] = sym;
        else if (sym == 256) {
          lpos = pos, lm = null;
          break;
        } else {
          var add = sym - 254;
          if (sym > 264) {
            var i = sym - 257, b = fleb[i];
            add = bits(dat, pos, (1 << b) - 1) + fl[i];
            pos += b;
          }
          var d = dm[bits16(dat, pos) & dms], dsym = d >> 4;
          if (!d)
            err(3);
          pos += d & 15;
          var dt = fd[dsym];
          if (dsym > 3) {
            var b = fdeb[dsym];
            dt += bits16(dat, pos) & (1 << b) - 1, pos += b;
          }
          if (pos > tbts) {
            if (noSt)
              err(0);
            break;
          }
          if (resize)
            cbuf(bt + 131072);
          var end = bt + add;
          if (bt < dt) {
            var shift = dl - dt, dend = Math.min(dt, end);
            if (shift + bt < 0)
              err(3);
            for (; bt < dend; ++bt)
              buf[bt] = dict[shift + bt];
          }
          for (; bt < end; ++bt)
            buf[bt] = buf[bt - dt];
        }
      }
      st.l = lm, st.p = lpos, st.b = bt, st.f = final;
      if (lm)
        final = 1, st.m = lbt, st.d = dm, st.n = dbt;
    } while (!final);
    return bt != buf.length && noBuf ? slc(buf, 0, bt) : buf.subarray(0, bt);
  };
  var et = /* @__PURE__ */ new u8(0);
  var gzs = function(d) {
    if (d[0] != 31 || d[1] != 139 || d[2] != 8)
      err(6, "invalid gzip data");
    var flg = d[3];
    var st = 10;
    if (flg & 4)
      st += (d[10] | d[11] << 8) + 2;
    for (var zs = (flg >> 3 & 1) + (flg >> 4 & 1); zs > 0; zs -= !d[st++])
      ;
    return st + (flg & 2);
  };
  var gzl = function(d) {
    var l = d.length;
    return (d[l - 4] | d[l - 3] << 8 | d[l - 2] << 16 | d[l - 1] << 24) >>> 0;
  };
  var zls = function(d, dict) {
    if ((d[0] & 15) != 8 || d[0] >> 4 > 7 || (d[0] << 8 | d[1]) % 31)
      err(6, "invalid zlib data");
    if ((d[1] >> 5 & 1) == +!dict)
      err(6, "invalid zlib data: " + (d[1] & 32 ? "need" : "unexpected") + " dictionary");
    return (d[1] >> 3 & 4) + 2;
  };
  function inflateSync(data, opts) {
    return inflt(data, { i: 2 }, opts && opts.out, opts && opts.dictionary);
  }
  function gunzipSync(data, opts) {
    var st = gzs(data);
    if (st + 8 > data.length)
      err(6, "invalid gzip data");
    return inflt(data.subarray(st, -8), { i: 2 }, opts && opts.out || new u8(gzl(data)), opts && opts.dictionary);
  }
  function unzlibSync(data, opts) {
    return inflt(data.subarray(zls(data, opts && opts.dictionary), -4), { i: 2 }, opts && opts.out, opts && opts.dictionary);
  }
  var td = typeof TextDecoder != "undefined" && /* @__PURE__ */ new TextDecoder();
  var tds = 0;
  try {
    td.decode(et, { stream: true });
    tds = 1;
  } catch (e) {
  }

  // src/shim/polyfills.ts
  var TextEncoderPolyfill = class {
    encoding = "utf-8";
    encode(text = "") {
      const bytes = [];
      for (const character of text) {
        let code = character.codePointAt(0) ?? 0;
        if (code < 128) {
          bytes.push(code);
        } else if (code < 2048) {
          bytes.push(192 | code >> 6, 128 | code & 63);
        } else if (code < 65536) {
          bytes.push(224 | code >> 12, 128 | code >> 6 & 63, 128 | code & 63);
        } else {
          code = Math.min(code, 1114111);
          bytes.push(
            240 | code >> 18,
            128 | code >> 12 & 63,
            128 | code >> 6 & 63,
            128 | code & 63
          );
        }
      }
      return Uint8Array.from(bytes);
    }
  };
  var TextDecoderPolyfill = class {
    encoding = "utf-8";
    decode(input) {
      if (!input) return "";
      const bytes = input instanceof ArrayBuffer ? new Uint8Array(input) : new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
      let text = "";
      for (let index = 0; index < bytes.length; ) {
        const first = bytes[index++] ?? 0;
        let code = first;
        const extra = first >= 240 ? 3 : first >= 224 ? 2 : first >= 192 ? 1 : 0;
        if (extra > 0) {
          code = first & 63 >> extra;
          for (let part = 0; part < extra; part += 1) {
            code = code << 6 | (bytes[index++] ?? 0) & 63;
          }
        }
        text += String.fromCodePoint(Math.min(code, 1114111));
      }
      return text;
    }
  };
  var BASE64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  var btoaPolyfill = (binary) => {
    let out = "";
    for (let index = 0; index < binary.length; index += 3) {
      const [a, b, c] = [0, 1, 2].map((offset) => binary.charCodeAt(index + offset));
      const triple = (a ?? 0) << 16 | (b || 0) << 8 | (c || 0);
      out += BASE64[triple >> 18 & 63];
      out += BASE64[triple >> 12 & 63];
      out += index + 1 < binary.length ? BASE64[triple >> 6 & 63] : "=";
      out += index + 2 < binary.length ? BASE64[triple & 63] : "=";
    }
    return out;
  };
  var atobPolyfill = (base64) => {
    const clean = base64.replace(/[^A-Za-z0-9+/]/g, "");
    let out = "";
    for (let index = 0; index < clean.length; index += 4) {
      const values = [0, 1, 2, 3].map((offset) => BASE64.indexOf(clean[index + offset] ?? "A"));
      const triple = (values[0] ?? 0) << 18 | (values[1] ?? 0) << 12 | (values[2] ?? 0) << 6 | (values[3] ?? 0);
      out += String.fromCharCode(triple >> 16 & 255);
      if (index + 2 < clean.length) out += String.fromCharCode(triple >> 8 & 255);
      if (index + 3 < clean.length) out += String.fromCharCode(triple & 255);
    }
    return out;
  };
  var encodeForm = (text) => encodeURIComponent(text).replace(/[!'()~]/g, (character) => \`%\${character.charCodeAt(0).toString(16).toUpperCase()}\`).replace(/%20/g, "+");
  var decodeForm = (text) => {
    try {
      return decodeURIComponent(text.replace(/\\+/g, " "));
    } catch {
      return text;
    }
  };
  var URLSearchParamsPolyfill = class _URLSearchParamsPolyfill {
    #pairs = [];
    #onChange;
    constructor(init, onChange) {
      if (typeof init === "string") {
        this.#parse(init);
      } else if (init instanceof _URLSearchParamsPolyfill) {
        this.#pairs = [...init.entries()];
      } else if (Array.isArray(init)) {
        this.#pairs = init.map(([key, value]) => [String(key), String(value)]);
      } else if (init) {
        this.#pairs = Object.entries(init).map(([key, value]) => [key, String(value)]);
      }
      this.#onChange = onChange;
    }
    #parse(text) {
      this.#pairs = text.replace(/^\\?/, "").split("&").filter(Boolean).map((pair) => {
        const [key = "", ...rest] = pair.split("=");
        return [decodeForm(key), decodeForm(rest.join("="))];
      });
    }
    /** Used by URL when its \`search\` changes. */
    replaceWith(text) {
      this.#parse(text);
    }
    #changed() {
      this.#onChange?.();
    }
    append(key, value) {
      this.#pairs.push([String(key), String(value)]);
      this.#changed();
    }
    /** Replaces the first \`key\` in place and drops any others. */
    set(key, value) {
      const index = this.#pairs.findIndex(([name]) => name === key);
      if (index === -1) {
        this.#pairs.push([String(key), String(value)]);
      } else {
        this.#pairs[index] = [String(key), String(value)];
        this.#pairs = this.#pairs.filter(([name], position) => name !== key || position === index);
      }
      this.#changed();
    }
    get(key) {
      return this.#pairs.find(([name]) => name === key)?.[1] ?? null;
    }
    getAll(key) {
      return this.#pairs.filter(([name]) => name === key).map(([, value]) => value);
    }
    has(key) {
      return this.#pairs.some(([name]) => name === key);
    }
    delete(key) {
      this.#pairs = this.#pairs.filter(([name]) => name !== key);
      this.#changed();
    }
    forEach(callback) {
      for (const [key, value] of this.#pairs) callback(value, key);
    }
    entries() {
      return this.#pairs.map(([key, value]) => [key, value])[Symbol.iterator]();
    }
    keys() {
      return this.#pairs.map(([key]) => key)[Symbol.iterator]();
    }
    values() {
      return this.#pairs.map(([, value]) => value)[Symbol.iterator]();
    }
    [Symbol.iterator]() {
      return this.entries();
    }
    get size() {
      return this.#pairs.length;
    }
    toString() {
      return this.#pairs.map(([key, value]) => \`\${encodeForm(key)}=\${encodeForm(value)}\`).join("&");
    }
  };
  var ABSOLUTE = /^([a-zA-Z][a-zA-Z0-9+.-]*:)(\\/\\/([^/?#]*))?([^?#]*)(\\?[^#]*)?(#.*)?$/;
  var normalizePath = (path) => {
    const output = [];
    for (const segment of path.split("/")) {
      if (segment === "..") output.pop();
      else if (segment !== ".") output.push(segment);
    }
    const joined = output.join("/");
    const trailing = /\\/\\.\\.?$/.test(path) ? "/" : "";
    return (joined.startsWith("/") ? joined : \`/\${joined}\`) + trailing;
  };
  var URLPolyfill = class _URLPolyfill {
    protocol = "";
    username = "";
    password = "";
    hostname = "";
    port = "";
    pathname = "/";
    hash = "";
    #search = "";
    searchParams;
    constructor(input, base) {
      const text = String(input).trim();
      const match = ABSOLUTE.exec(text);
      if (match) {
        this.#assign(match);
      } else {
        if (base === void 0) throw new TypeError(\`Invalid URL: \${text}\`);
        const root = new _URLPolyfill(String(base));
        this.protocol = root.protocol;
        this.username = root.username;
        this.password = root.password;
        this.hostname = root.hostname;
        this.port = root.port;
        if (text.startsWith("//")) {
          this.#assign(ABSOLUTE.exec(\`\${root.protocol}\${text}\`));
        } else if (text.startsWith("/")) {
          this.#setPathQueryHash(text);
        } else if (text.startsWith("?") || text === "") {
          this.pathname = root.pathname;
          this.#setPathQueryHash(\`\${root.pathname}\${text || root.search}\`);
        } else if (text.startsWith("#")) {
          this.pathname = root.pathname;
          this.#search = root.search;
          this.hash = text;
        } else {
          const directory = root.pathname.replace(/[^/]*$/, "");
          this.#setPathQueryHash(\`\${directory}\${text}\`);
        }
      }
      this.searchParams = new URLSearchParamsPolyfill(this.#search, () => {
        const query = this.searchParams.toString();
        this.#search = query ? \`?\${query}\` : "";
      });
    }
    #assign(match) {
      const [, protocol = "", , authority = "", path = "", search = "", hash = ""] = match;
      this.protocol = protocol.toLowerCase();
      const at = authority.lastIndexOf("@");
      const credentials = at >= 0 ? authority.slice(0, at) : "";
      const hostPort = at >= 0 ? authority.slice(at + 1) : authority;
      [this.username = "", this.password = ""] = credentials ? credentials.split(":") : [];
      const portMatch = /^(.*?)(?::(\\d+))?$/.exec(hostPort);
      this.hostname = (portMatch?.[1] ?? "").toLowerCase();
      this.port = portMatch?.[2] ?? "";
      this.pathname = path ? normalizePath(path) : authority ? "/" : "";
      this.#search = search === "?" ? "" : search;
      this.hash = hash === "#" ? "" : hash;
    }
    #setPathQueryHash(text) {
      const [, path = "", search = "", hash = ""] = /^([^?#]*)(\\?[^#]*)?(#.*)?$/.exec(text) ?? [];
      this.pathname = normalizePath(path);
      this.#search = search === "?" ? "" : search;
      this.hash = hash === "#" ? "" : hash;
    }
    get search() {
      return this.#search;
    }
    set search(value) {
      const text = String(value);
      this.#search = text && !text.startsWith("?") ? \`?\${text}\` : text === "?" ? "" : text;
      this.searchParams.replaceWith(this.#search);
    }
    get host() {
      return this.port ? \`\${this.hostname}:\${this.port}\` : this.hostname;
    }
    get origin() {
      return \`\${this.protocol}//\${this.host}\`;
    }
    get href() {
      const credentials = this.username ? \`\${this.username}\${this.password ? \`:\${this.password}\` : ""}@\` : "";
      return \`\${this.protocol}//\${credentials}\${this.host}\${this.pathname}\${this.#search}\${this.hash}\`;
    }
    toString() {
      return this.href;
    }
    toJSON() {
      return this.href;
    }
  };
  var HeadersPolyfill = class _HeadersPolyfill {
    #map = /* @__PURE__ */ new Map();
    constructor(init) {
      if (init instanceof _HeadersPolyfill) init.forEach((value, key) => this.set(key, value));
      else if (Array.isArray(init)) for (const [key, value] of init) this.append(key, value);
      else if (init) for (const [key, value] of Object.entries(init)) this.set(key, String(value));
    }
    get(name) {
      return this.#map.get(name.toLowerCase()) ?? null;
    }
    set(name, value) {
      this.#map.set(name.toLowerCase(), String(value));
    }
    append(name, value) {
      const key = name.toLowerCase();
      const existing = this.#map.get(key);
      this.#map.set(key, existing ? \`\${existing}, \${value}\` : String(value));
    }
    has(name) {
      return this.#map.has(name.toLowerCase());
    }
    delete(name) {
      this.#map.delete(name.toLowerCase());
    }
    forEach(callback) {
      for (const [key, value] of this.#map) callback(value, key);
    }
    entries() {
      return this.#map.entries();
    }
    [Symbol.iterator]() {
      return this.#map.entries();
    }
    toRecord() {
      return Object.fromEntries(this.#map);
    }
  };
  var streamOf = (bytes) => ({
    bytes,
    pipeThrough: (transform) => streamOf(transform.transform(bytes))
  });
  var toBytes = (part) => {
    if (typeof part === "string") return new TextEncoderPolyfill().encode(part);
    if (part instanceof Uint8Array) return part;
    if (part instanceof ArrayBuffer) return new Uint8Array(part);
    if (ArrayBuffer.isView(part))
      return new Uint8Array(part.buffer, part.byteOffset, part.byteLength);
    if (part instanceof BlobPolyfill) return part.bytes;
    return new TextEncoderPolyfill().encode(String(part));
  };
  var concat = (chunks) => {
    const out = new Uint8Array(chunks.reduce((total, chunk) => total + chunk.length, 0));
    let offset = 0;
    for (const chunk of chunks) {
      out.set(chunk, offset);
      offset += chunk.length;
    }
    return out;
  };
  var BlobPolyfill = class {
    bytes;
    type;
    constructor(parts = [], options = {}) {
      this.bytes = concat(parts.map(toBytes));
      this.type = options.type ?? "";
    }
    get size() {
      return this.bytes.length;
    }
    stream() {
      return streamOf(this.bytes);
    }
    async text() {
      return new TextDecoderPolyfill().decode(this.bytes);
    }
    async arrayBuffer() {
      return this.bytes.slice().buffer;
    }
  };
  var DecompressionStreamPolyfill = class {
    #format;
    constructor(format2) {
      this.#format = format2;
    }
    transform(bytes) {
      if (this.#format === "gzip") return gunzipSync(bytes);
      if (this.#format === "deflate-raw") return inflateSync(bytes);
      return unzlibSync(bytes);
    }
  };
  var ResponsePolyfill = class _ResponsePolyfill {
    status;
    statusText;
    headers;
    url;
    #bytes;
    bodyUsed = false;
    constructor(body, init = {}) {
      this.#bytes = body === null || body === void 0 ? new Uint8Array() : typeof body === "object" && "pipeThrough" in body ? body.bytes : toBytes(body);
      this.status = init.status ?? 200;
      this.statusText = init.statusText ?? "";
      this.headers = new HeadersPolyfill(init.headers);
      this.url = init.url ?? "";
    }
    get ok() {
      return this.status >= 200 && this.status < 300;
    }
    async text() {
      this.bodyUsed = true;
      return new TextDecoderPolyfill().decode(this.#bytes);
    }
    async json() {
      return JSON.parse(await this.text());
    }
    async arrayBuffer() {
      this.bodyUsed = true;
      return this.#bytes.slice().buffer;
    }
    async blob() {
      return new BlobPolyfill([this.#bytes]);
    }
    clone() {
      return new _ResponsePolyfill(this.#bytes, this);
    }
  };
  var nextTimer = 1;
  var cancelled = /* @__PURE__ */ new Set();
  var setTimeoutPolyfill = (callback, ms = 0, ...args) => {
    const id = nextTimer++;
    void raygun.time.sleep(Number(ms) || 0).then(() => {
      if (cancelled.delete(id)) return;
      callback(...args);
    });
    return id;
  };
  var clearTimeoutPolyfill = (id) => {
    if (id !== void 0) cancelled.add(id);
  };
  var setIntervalPolyfill = (callback, ms = 0, ...args) => {
    const id = nextTimer++;
    const tick = () => {
      void raygun.time.sleep(Math.max(Number(ms) || 0, 10)).then(() => {
        if (cancelled.delete(id)) return;
        callback(...args);
        tick();
      });
    };
    tick();
    return id;
  };
  var AbortControllerPolyfill = class {
    signal = {
      aborted: false,
      reason: void 0,
      addEventListener() {
      },
      removeEventListener() {
      }
    };
    abort(reason) {
      this.signal.aborted = true;
      this.signal.reason = reason;
    }
  };
  var installPolyfills = (target = globalThis) => {
    const provide = (name, value) => {
      if (target[name] === void 0) target[name] = value;
    };
    provide("TextEncoder", TextEncoderPolyfill);
    provide("TextDecoder", TextDecoderPolyfill);
    provide("atob", atobPolyfill);
    provide("btoa", btoaPolyfill);
    provide("URL", URLPolyfill);
    provide("URLSearchParams", URLSearchParamsPolyfill);
    provide("Headers", HeadersPolyfill);
    provide("Blob", BlobPolyfill);
    provide("DecompressionStream", DecompressionStreamPolyfill);
    provide("Response", ResponsePolyfill);
    provide("setTimeout", setTimeoutPolyfill);
    provide("clearTimeout", clearTimeoutPolyfill);
    provide("setInterval", setIntervalPolyfill);
    provide("clearInterval", clearTimeoutPolyfill);
    provide("AbortController", AbortControllerPolyfill);
    provide("queueMicrotask", (callback) => void Promise.resolve().then(callback));
  };

  // src/shim/providers.ts
  var KIND_MAP = {
    metadata: "music.metadata",
    streaming: "music.stream",
    lyrics: "lyrics"
  };
  var providerId = (id) => id.toLowerCase().replace(/[^a-z0-9_-]+/g, "-").replace(/^[^a-z0-9]+/, "").slice(0, 64) || "provider";
  var call = (provider, method, ...args) => {
    const fn = provider[method];
    if (typeof fn !== "function") {
      throw new Error(\`\${provider.name} has no \${method}\`);
    }
    return Promise.resolve(fn.apply(provider, args));
  };
  var has = (provider, method) => typeof provider[method] === "function";
  var pickArtwork = (artwork) => {
    const items = (artwork?.items ?? []).filter((item) => /^https?:\\/\\//.test(item.url));
    const covers = items.filter((item) => !item.purpose || item.purpose === "cover");
    const pool = covers.length > 0 ? covers : items;
    const best = [...pool].sort((a, b) => (b.width ?? 0) - (a.width ?? 0))[0];
    return best?.url ?? null;
  };
  var toRaygunTrack = (track) => ({
    id: track.source?.id,
    title: track.title,
    artist: track.artists?.map((artist) => artist.name).join(", ") || null,
    album: track.album?.title ?? null,
    durationMs: track.durationMs ?? null,
    artworkUrl: pickArtwork(track.artwork)
  });
  var toNuclearTrack = (query) => ({
    title: query.title,
    artists: query.artist ? [{ name: query.artist, roles: ["main"] }] : [],
    album: query.album ? { title: query.album, source: { provider: "raygun", id: "" } } : void 0,
    durationMs: query.durationMs ?? void 0,
    source: { provider: "raygun", id: "" }
  });
  var playable = (stream) => stream && (stream.protocol === "http" || stream.protocol === "https") && /^https?:\\/\\//.test(stream.url);
  var toRaygunLyrics = (lyrics) => {
    if (!lyrics || lyrics.type === "instrumental") return null;
    const lineText = (line) => line.segments.map((segment) => segment.text).join("").trim();
    if (lyrics.type === "plain") {
      const plain = lyrics.sections.map((section) => section.lines.map(lineText).join("\\n")).filter(Boolean).join("\\n\\n");
      return plain ? { plain } : null;
    }
    const synced = lyrics.sections.flatMap(
      (section) => section.lines.filter((line) => typeof line.startMs === "number").map((line) => ({ timeMs: line.startMs, text: lineText(line) }))
    );
    return synced.length > 0 ? { synced } : null;
  };
  var candidates = /* @__PURE__ */ new Map();
  var wrapProvider = (provider) => {
    const kind = KIND_MAP[provider.kind];
    const id = providerId(provider.id);
    const name = provider.name;
    if (kind === "music.metadata") {
      return {
        kind,
        id,
        name,
        async search({ query, limit }) {
          let tracks = [];
          if (has(provider, "searchTracks")) {
            tracks = await call(provider, "searchTracks", { query, limit });
          } else if (has(provider, "search")) {
            const results = await call(provider, "search", {
              query,
              types: ["tracks"],
              limit
            });
            tracks = results?.tracks ?? [];
          }
          return { tracks: (tracks ?? []).slice(0, limit).map(toRaygunTrack) };
        }
      };
    }
    if (kind === "music.stream") {
      return {
        kind,
        id,
        name,
        async candidates(query) {
          const found = has(provider, "searchForTrackV2") ? await call(
            provider,
            "searchForTrackV2",
            toNuclearTrack(query)
          ) : await call(
            provider,
            "searchForTrack",
            query.artist ?? "",
            query.title,
            query.album ?? void 0
          );
          const list = (found ?? []).filter((candidate) => !candidate.failed).slice(0, query.limit);
          for (const candidate of list) candidates.set(candidate.id, candidate);
          return {
            candidates: list.map((candidate) => ({
              id: candidate.id,
              title: candidate.title,
              durationMs: candidate.durationMs ?? null,
              quality: candidate.stream?.qualityLabel ?? null,
              source: playable(candidate.stream) ? { kind: "url", url: candidate.stream.url, headers: {} } : null
            }))
          };
        },
        async resolve({ id: candidateId }) {
          const known = candidates.get(candidateId);
          const stream = has(provider, "getStreamUrlV2") && known ? await call(provider, "getStreamUrlV2", known) : await call(provider, "getStreamUrl", candidateId);
          if (!playable(stream)) {
            throw new Error(
              \`\${name} returned a \${stream?.protocol ?? "missing"} stream Raygun can't play\`
            );
          }
          return { source: { kind: "url", url: stream.url, headers: {} } };
        }
      };
    }
    if (kind === "lyrics") {
      return {
        kind,
        id,
        name,
        async lyrics(query) {
          const lyrics = await call(
            provider,
            "getLyrics",
            toNuclearTrack(query),
            {}
          );
          return toRaygunLyrics(lyrics);
        }
      };
    }
    return null;
  };

  // src/shim/api.ts
  var STUBBED_DOMAINS = [
    "Queue",
    "Playback",
    "Favorites",
    "Playlists",
    "Dashboard",
    "Discovery",
    "Events",
    "Shell",
    "Navigation",
    "Ytdlp",
    "Metadata",
    "Streaming",
    "Lyrics"
  ];
  var unsupported = (domain) => new Proxy(
    {},
    {
      get: (_target, property) => {
        if (property === Symbol.toPrimitive || property === "then") return void 0;
        return () => {
          throw new Error(\`\${domain} isn't available to Nuclear plugins in Raygun yet\`);
        };
      }
    }
  );
  var headersRecord = (headers) => {
    if (!headers) return {};
    if (headers instanceof HeadersPolyfill) return headers.toRecord();
    if (Array.isArray(headers)) return Object.fromEntries(headers);
    if (typeof headers.forEach === "function") {
      const record = {};
      headers.forEach(
        (value, key) => record[key] = value
      );
      return record;
    }
    return Object.fromEntries(
      Object.entries(headers).map(([key, value]) => [key, String(value)])
    );
  };
  var pluginFetch = async (input, init = {}) => {
    const url = String(
      typeof input === "object" && input !== null && "url" in input ? input.url : input
    );
    const response = await raygun.http.fetch(url, {
      method: init.method ?? "GET",
      headers: headersRecord(init.headers),
      body: typeof init.body === "string" ? init.body : null
    });
    return new ResponsePolyfill(await response.text(), {
      status: response.status,
      headers: response.headers,
      url: response.url
    });
  };
  var format = (args) => args.map(
    (arg) => typeof arg === "string" ? arg : arg instanceof Error ? arg.message : JSON.stringify(arg)
  );
  var createApi = (plugin) => {
    const defaults = /* @__PURE__ */ new Map();
    const registered = /* @__PURE__ */ new Set();
    const key = (id) => \`nuclear-setting:\${id}\`;
    const Logger = {
      debug: (...args) => raygun.log.debug(...format(args)),
      info: (...args) => raygun.log.info(...format(args)),
      log: (...args) => raygun.log.info(...format(args)),
      warn: (...args) => raygun.log.warn(...format(args)),
      error: (...args) => raygun.log.error(...format(args))
    };
    const Settings = {
      async register(definitions) {
        for (const definition of definitions ?? []) defaults.set(definition.id, definition.default);
        return { registered: (definitions ?? []).map((definition) => definition.id) };
      },
      async get(id) {
        const stored = raygun.storage.get(key(id));
        return stored ?? defaults.get(id);
      },
      async set(id, value) {
        raygun.storage.set(key(id), value ?? null);
      },
      async getGlobal() {
        return void 0;
      },
      async setGlobal() {
      },
      subscribe: () => () => {
      }
    };
    const Providers = {
      register(provider) {
        const wrapped = wrapProvider(provider);
        if (!wrapped) {
          Logger.warn(\`\${plugin.name}: "\${provider.kind}" providers aren't supported in Raygun\`);
          return provider.id;
        }
        if (!registered.has(wrapped.id)) {
          registered.add(wrapped.id);
          raygun.providers.register(wrapped);
        }
        return provider.id;
      },
      unregister: () => false,
      list: () => [],
      get: () => void 0,
      getActive: () => void 0,
      setActive() {
      },
      subscribe: () => () => {
      },
      clear() {
      },
      supportedKinds: Object.keys(KIND_MAP)
    };
    const api = {
      Http: { fetch: pluginFetch },
      Logger,
      Settings,
      Providers
    };
    for (const domain of STUBBED_DOMAINS) api[domain] = unsupported(domain);
    return api;
  };

  // src/shim/run.ts
  var EXTENSIONS = ["", ".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs", ".json"];
  var dirname = (path) => path.replace(/\\/[^/]*$/, "") || ".";
  var join = (base, relative) => {
    const parts = [];
    for (const segment of \`\${base}/\${relative}\`.split("/")) {
      if (segment === "..") parts.pop();
      else if (segment && segment !== ".") parts.push(segment);
    }
    return parts.join("/");
  };
  var inert = new Proxy(function inertStub() {
  }, {
    get: (_target, property) => property === Symbol.toPrimitive ? () => "" : inert,
    apply: () => inert,
    construct: () => inert
  });
  var createRequire = (definitions, bare) => {
    const cache = /* @__PURE__ */ new Map();
    const load = (path) => {
      const cached = cache.get(path);
      if (cached) return cached.exports;
      const module = { exports: {} };
      cache.set(path, module);
      const define = definitions[path];
      define(module.exports, (id) => requireFrom(path, id), module);
      return module.exports;
    };
    const resolve = (from, id) => {
      const base = join(dirname(from), id);
      for (const candidate of [
        ...EXTENSIONS.map((extension) => \`\${base}\${extension}\`),
        ...EXTENSIONS.slice(1).map((extension) => \`\${base}/index\${extension}\`)
      ]) {
        if (candidate in definitions) return candidate;
      }
      throw new Error(\`Cannot find module '\${id}' from '\${from}'\`);
    };
    const requireFrom = (from, id) => {
      if (id.startsWith(".")) return load(resolve(from, id));
      if (id in bare) return bare[id];
      if (id === "react" || id.startsWith("react/") || id.startsWith("@nuclearplayer/ui"))
        return inert;
      throw new Error(\`Nuclear plugin needs '\${id}', which Raygun doesn't provide\`);
    };
    return { load };
  };
  var run = (options) => {
    const api = createApi(options.plugin);
    const { load } = createRequire(options.definitions, {
      "@nuclearplayer/plugin-sdk": { NuclearPluginAPI: class NuclearPluginAPI {
      } },
      "@nuclearplayer/model": {}
    });
    const exports = load(options.entry);
    const plugin = exports?.default ?? exports;
    if (!plugin || typeof plugin !== "object") {
      throw new Error("Nuclear plugin must export a default object");
    }
    const ready = (async () => {
      await plugin.onLoad?.(api);
      await plugin.onEnable?.(api);
    })();
    globalThis.__rg_ready = ready;
    return ready;
  };

  // src/shim/index.ts
  installPolyfills();
  return __toCommonJS(index_exports);
})();
`;var l;(function(e){e[e.NONE=0]="NONE";let r=1;e[e._abstract=r]="_abstract";let o=r+1;e[e._accessor=o]="_accessor";let i=o+1;e[e._as=i]="_as";let c=i+1;e[e._assert=c]="_assert";let p=c+1;e[e._asserts=p]="_asserts";let h=p+1;e[e._async=h]="_async";let x=h+1;e[e._await=x]="_await";let k=x+1;e[e._checks=k]="_checks";let y=k+1;e[e._constructor=y]="_constructor";let w=y+1;e[e._declare=w]="_declare";let I=w+1;e[e._enum=I]="_enum";let A=I+1;e[e._exports=A]="_exports";let O=A+1;e[e._from=O]="_from";let E=O+1;e[e._get=E]="_get";let $=E+1;e[e._global=$]="_global";let Q=$+1;e[e._implements=Q]="_implements";let Se=Q+1;e[e._infer=Se]="_infer";let ge=Se+1;e[e._interface=ge]="_interface";let ie=ge+1;e[e._is=ie]="_is";let Fe=ie+1;e[e._keyof=Fe]="_keyof";let Me=Fe+1;e[e._mixins=Me]="_mixins";let Be=Me+1;e[e._module=Be]="_module";let qe=Be+1;e[e._namespace=qe]="_namespace";let $e=qe+1;e[e._of=$e]="_of";let Ue=$e+1;e[e._opaque=Ue]="_opaque";let He=Ue+1;e[e._out=He]="_out";let Ve=He+1;e[e._override=Ve]="_override";let We=Ve+1;e[e._private=We]="_private";let ze=We+1;e[e._protected=ze]="_protected";let Ge=ze+1;e[e._proto=Ge]="_proto";let Xe=Ge+1;e[e._public=Xe]="_public";let Je=Xe+1;e[e._readonly=Je]="_readonly";let Ke=Je+1;e[e._require=Ke]="_require";let Qe=Ke+1;e[e._satisfies=Qe]="_satisfies";let Ye=Qe+1;e[e._set=Ye]="_set";let Ze=Ye+1;e[e._static=Ze]="_static";let et=Ze+1;e[e._symbol=et]="_symbol";let tt=et+1;e[e._type=tt]="_type";let nt=tt+1;e[e._unique=nt]="_unique";let dt=nt+1;e[e._using=dt]="_using"})(l||(l={}));var t;(function(e){e[e.PRECEDENCE_MASK=15]="PRECEDENCE_MASK";let r=16;e[e.IS_KEYWORD=r]="IS_KEYWORD";let o=32;e[e.IS_ASSIGN=o]="IS_ASSIGN";let i=64;e[e.IS_RIGHT_ASSOCIATIVE=i]="IS_RIGHT_ASSOCIATIVE";let c=128;e[e.IS_PREFIX=c]="IS_PREFIX";let p=256;e[e.IS_POSTFIX=p]="IS_POSTFIX";let h=512;e[e.IS_EXPRESSION_START=h]="IS_EXPRESSION_START";let x=512;e[e.num=x]="num";let k=1536;e[e.bigint=k]="bigint";let y=2560;e[e.decimal=y]="decimal";let w=3584;e[e.regexp=w]="regexp";let I=4608;e[e.string=I]="string";let A=5632;e[e.name=A]="name";let O=6144;e[e.eof=O]="eof";let E=7680;e[e.bracketL=E]="bracketL";let $=8192;e[e.bracketR=$]="bracketR";let Q=9728;e[e.braceL=Q]="braceL";let Se=10752;e[e.braceBarL=Se]="braceBarL";let ge=11264;e[e.braceR=ge]="braceR";let ie=12288;e[e.braceBarR=ie]="braceBarR";let Fe=13824;e[e.parenL=Fe]="parenL";let Me=14336;e[e.parenR=Me]="parenR";let Be=15360;e[e.comma=Be]="comma";let qe=16384;e[e.semi=qe]="semi";let $e=17408;e[e.colon=$e]="colon";let Ue=18432;e[e.doubleColon=Ue]="doubleColon";let He=19456;e[e.dot=He]="dot";let Ve=20480;e[e.question=Ve]="question";let We=21504;e[e.questionDot=We]="questionDot";let ze=22528;e[e.arrow=ze]="arrow";let Ge=23552;e[e.template=Ge]="template";let Xe=24576;e[e.ellipsis=Xe]="ellipsis";let Je=25600;e[e.backQuote=Je]="backQuote";let Ke=27136;e[e.dollarBraceL=Ke]="dollarBraceL";let Qe=27648;e[e.at=Qe]="at";let Ye=29184;e[e.hash=Ye]="hash";let Ze=29728;e[e.eq=Ze]="eq";let et=30752;e[e.assign=et]="assign";let tt=32640;e[e.preIncDec=tt]="preIncDec";let nt=33664;e[e.postIncDec=nt]="postIncDec";let dt=34432;e[e.bang=dt]="bang";let Bn=35456;e[e.tilde=Bn]="tilde";let qn=35841;e[e.pipeline=qn]="pipeline";let $n=36866;e[e.nullishCoalescing=$n]="nullishCoalescing";let Un=37890;e[e.logicalOR=Un]="logicalOR";let Hn=38915;e[e.logicalAND=Hn]="logicalAND";let Vn=39940;e[e.bitwiseOR=Vn]="bitwiseOR";let Wn=40965;e[e.bitwiseXOR=Wn]="bitwiseXOR";let zn=41990;e[e.bitwiseAND=zn]="bitwiseAND";let Gn=43015;e[e.equality=Gn]="equality";let Xn=44040;e[e.lessThan=Xn]="lessThan";let Jn=45064;e[e.greaterThan=Jn]="greaterThan";let Kn=46088;e[e.relationalOrEqual=Kn]="relationalOrEqual";let Qn=47113;e[e.bitShiftL=Qn]="bitShiftL";let Yn=48137;e[e.bitShiftR=Yn]="bitShiftR";let Zn=49802;e[e.plus=Zn]="plus";let er=50826;e[e.minus=er]="minus";let tr=51723;e[e.modulo=tr]="modulo";let nr=52235;e[e.star=nr]="star";let rr=53259;e[e.slash=rr]="slash";let sr=54348;e[e.exponent=sr]="exponent";let or=55296;e[e.jsxName=or]="jsxName";let ir=56320;e[e.jsxText=ir]="jsxText";let ar=57344;e[e.jsxEmptyText=ar]="jsxEmptyText";let cr=58880;e[e.jsxTagStart=cr]="jsxTagStart";let lr=59392;e[e.jsxTagEnd=lr]="jsxTagEnd";let ur=60928;e[e.typeParameterStart=ur]="typeParameterStart";let pr=61440;e[e.nonNullAssertion=pr]="nonNullAssertion";let fr=62480;e[e._break=fr]="_break";let hr=63504;e[e._case=hr]="_case";let mr=64528;e[e._catch=mr]="_catch";let dr=65552;e[e._continue=dr]="_continue";let kr=66576;e[e._debugger=kr]="_debugger";let gr=67600;e[e._default=gr]="_default";let xr=68624;e[e._do=xr]="_do";let yr=69648;e[e._else=yr]="_else";let _r=70672;e[e._finally=_r]="_finally";let br=71696;e[e._for=br]="_for";let Tr=73232;e[e._function=Tr]="_function";let Ir=73744;e[e._if=Ir]="_if";let wr=74768;e[e._return=wr]="_return";let vr=75792;e[e._switch=vr]="_switch";let Sr=77456;e[e._throw=Sr]="_throw";let Er=77840;e[e._try=Er]="_try";let Ar=78864;e[e._var=Ar]="_var";let Pr=79888;e[e._let=Pr]="_let";let Cr=80912;e[e._const=Cr]="_const";let Nr=81936;e[e._while=Nr]="_while";let Rr=82960;e[e._with=Rr]="_with";let Lr=84496;e[e._new=Lr]="_new";let Dr=85520;e[e._this=Dr]="_this";let Or=86544;e[e._super=Or]="_super";let jr=87568;e[e._class=jr]="_class";let Fr=88080;e[e._extends=Fr]="_extends";let Mr=89104;e[e._export=Mr]="_export";let Br=90640;e[e._import=Br]="_import";let qr=91664;e[e._yield=qr]="_yield";let $r=92688;e[e._null=$r]="_null";let Ur=93712;e[e._true=Ur]="_true";let Hr=94736;e[e._false=Hr]="_false";let Vr=95256;e[e._in=Vr]="_in";let Wr=96280;e[e._instanceof=Wr]="_instanceof";let zr=97936;e[e._typeof=zr]="_typeof";let Gr=98960;e[e._void=Gr]="_void";let q1=99984;e[e._delete=q1]="_delete";let $1=100880;e[e._async=$1]="_async";let U1=101904;e[e._get=U1]="_get";let H1=102928;e[e._set=H1]="_set";let V1=103952;e[e._declare=V1]="_declare";let W1=104976;e[e._readonly=W1]="_readonly";let z1=106e3;e[e._abstract=z1]="_abstract";let G1=107024;e[e._static=G1]="_static";let X1=107536;e[e._public=X1]="_public";let J1=108560;e[e._private=J1]="_private";let K1=109584;e[e._protected=K1]="_protected";let Q1=110608;e[e._override=Q1]="_override";let Y1=112144;e[e._as=Y1]="_as";let Z1=113168;e[e._enum=Z1]="_enum";let ea=114192;e[e._type=ea]="_type";let ta=115216;e[e._implements=ta]="_implements"})(t||(t={}));function Jr(e){switch(e){case t.num:return"num";case t.bigint:return"bigint";case t.decimal:return"decimal";case t.regexp:return"regexp";case t.string:return"string";case t.name:return"name";case t.eof:return"eof";case t.bracketL:return"[";case t.bracketR:return"]";case t.braceL:return"{";case t.braceBarL:return"{|";case t.braceR:return"}";case t.braceBarR:return"|}";case t.parenL:return"(";case t.parenR:return")";case t.comma:return",";case t.semi:return";";case t.colon:return":";case t.doubleColon:return"::";case t.dot:return".";case t.question:return"?";case t.questionDot:return"?.";case t.arrow:return"=>";case t.template:return"template";case t.ellipsis:return"...";case t.backQuote:return"`";case t.dollarBraceL:return"${";case t.at:return"@";case t.hash:return"#";case t.eq:return"=";case t.assign:return"_=";case t.preIncDec:return"++/--";case t.postIncDec:return"++/--";case t.bang:return"!";case t.tilde:return"~";case t.pipeline:return"|>";case t.nullishCoalescing:return"??";case t.logicalOR:return"||";case t.logicalAND:return"&&";case t.bitwiseOR:return"|";case t.bitwiseXOR:return"^";case t.bitwiseAND:return"&";case t.equality:return"==/!=";case t.lessThan:return"<";case t.greaterThan:return">";case t.relationalOrEqual:return"<=/>=";case t.bitShiftL:return"<<";case t.bitShiftR:return">>/>>>";case t.plus:return"+";case t.minus:return"-";case t.modulo:return"%";case t.star:return"*";case t.slash:return"/";case t.exponent:return"**";case t.jsxName:return"jsxName";case t.jsxText:return"jsxText";case t.jsxEmptyText:return"jsxEmptyText";case t.jsxTagStart:return"jsxTagStart";case t.jsxTagEnd:return"jsxTagEnd";case t.typeParameterStart:return"typeParameterStart";case t.nonNullAssertion:return"nonNullAssertion";case t._break:return"break";case t._case:return"case";case t._catch:return"catch";case t._continue:return"continue";case t._debugger:return"debugger";case t._default:return"default";case t._do:return"do";case t._else:return"else";case t._finally:return"finally";case t._for:return"for";case t._function:return"function";case t._if:return"if";case t._return:return"return";case t._switch:return"switch";case t._throw:return"throw";case t._try:return"try";case t._var:return"var";case t._let:return"let";case t._const:return"const";case t._while:return"while";case t._with:return"with";case t._new:return"new";case t._this:return"this";case t._super:return"super";case t._class:return"class";case t._extends:return"extends";case t._export:return"export";case t._import:return"import";case t._yield:return"yield";case t._null:return"null";case t._true:return"true";case t._false:return"false";case t._in:return"in";case t._instanceof:return"instanceof";case t._typeof:return"typeof";case t._void:return"void";case t._delete:return"delete";case t._async:return"async";case t._get:return"get";case t._set:return"set";case t._declare:return"declare";case t._readonly:return"readonly";case t._abstract:return"abstract";case t._static:return"static";case t._public:return"public";case t._private:return"private";case t._protected:return"protected";case t._override:return"override";case t._as:return"as";case t._enum:return"enum";case t._type:return"type";case t._implements:return"implements";default:return""}}var ee=class{constructor(n,r,o){this.startTokenIndex=n,this.endTokenIndex=r,this.isFunctionScope=o}},Kr=class{constructor(n,r,o,i,c,p,h,x,k,y,w,I,A){this.potentialArrowAt=n,this.noAnonFunctionType=r,this.inDisallowConditionalTypesContext=o,this.tokensLength=i,this.scopesLength=c,this.pos=p,this.type=h,this.contextualKeyword=x,this.start=k,this.end=y,this.isType=w,this.scopeDepth=I,this.error=A}},kt=class e{constructor(){e.prototype.__init.call(this),e.prototype.__init2.call(this),e.prototype.__init3.call(this),e.prototype.__init4.call(this),e.prototype.__init5.call(this),e.prototype.__init6.call(this),e.prototype.__init7.call(this),e.prototype.__init8.call(this),e.prototype.__init9.call(this),e.prototype.__init10.call(this),e.prototype.__init11.call(this),e.prototype.__init12.call(this),e.prototype.__init13.call(this)}__init(){this.potentialArrowAt=-1}__init2(){this.noAnonFunctionType=!1}__init3(){this.inDisallowConditionalTypesContext=!1}__init4(){this.tokens=[]}__init5(){this.scopes=[]}__init6(){this.pos=0}__init7(){this.type=t.eof}__init8(){this.contextualKeyword=l.NONE}__init9(){this.start=0}__init10(){this.end=0}__init11(){this.isType=!1}__init12(){this.scopeDepth=0}__init13(){this.error=null}snapshot(){return new Kr(this.potentialArrowAt,this.noAnonFunctionType,this.inDisallowConditionalTypesContext,this.tokens.length,this.scopes.length,this.pos,this.type,this.contextualKeyword,this.start,this.end,this.isType,this.scopeDepth,this.error)}restoreFromSnapshot(n){this.potentialArrowAt=n.potentialArrowAt,this.noAnonFunctionType=n.noAnonFunctionType,this.inDisallowConditionalTypesContext=n.inDisallowConditionalTypesContext,this.tokens.length=n.tokensLength,this.scopes.length=n.scopesLength,this.pos=n.pos,this.type=n.type,this.contextualKeyword=n.contextualKeyword,this.start=n.start,this.end=n.end,this.isType=n.isType,this.scopeDepth=n.scopeDepth,this.error=n.error}};var u;(function(e){e[e.backSpace=8]="backSpace";let r=10;e[e.lineFeed=r]="lineFeed";let o=9;e[e.tab=o]="tab";let i=13;e[e.carriageReturn=i]="carriageReturn";let c=14;e[e.shiftOut=c]="shiftOut";let p=32;e[e.space=p]="space";let h=33;e[e.exclamationMark=h]="exclamationMark";let x=34;e[e.quotationMark=x]="quotationMark";let k=35;e[e.numberSign=k]="numberSign";let y=36;e[e.dollarSign=y]="dollarSign";let w=37;e[e.percentSign=w]="percentSign";let I=38;e[e.ampersand=I]="ampersand";let A=39;e[e.apostrophe=A]="apostrophe";let O=40;e[e.leftParenthesis=O]="leftParenthesis";let E=41;e[e.rightParenthesis=E]="rightParenthesis";let $=42;e[e.asterisk=$]="asterisk";let Q=43;e[e.plusSign=Q]="plusSign";let Se=44;e[e.comma=Se]="comma";let ge=45;e[e.dash=ge]="dash";let ie=46;e[e.dot=ie]="dot";let Fe=47;e[e.slash=Fe]="slash";let Me=48;e[e.digit0=Me]="digit0";let Be=49;e[e.digit1=Be]="digit1";let qe=50;e[e.digit2=qe]="digit2";let $e=51;e[e.digit3=$e]="digit3";let Ue=52;e[e.digit4=Ue]="digit4";let He=53;e[e.digit5=He]="digit5";let Ve=54;e[e.digit6=Ve]="digit6";let We=55;e[e.digit7=We]="digit7";let ze=56;e[e.digit8=ze]="digit8";let Ge=57;e[e.digit9=Ge]="digit9";let Xe=58;e[e.colon=Xe]="colon";let Je=59;e[e.semicolon=Je]="semicolon";let Ke=60;e[e.lessThan=Ke]="lessThan";let Qe=61;e[e.equalsTo=Qe]="equalsTo";let Ye=62;e[e.greaterThan=Ye]="greaterThan";let Ze=63;e[e.questionMark=Ze]="questionMark";let et=64;e[e.atSign=et]="atSign";let tt=65;e[e.uppercaseA=tt]="uppercaseA";let nt=66;e[e.uppercaseB=nt]="uppercaseB";let dt=67;e[e.uppercaseC=dt]="uppercaseC";let Bn=68;e[e.uppercaseD=Bn]="uppercaseD";let qn=69;e[e.uppercaseE=qn]="uppercaseE";let $n=70;e[e.uppercaseF=$n]="uppercaseF";let Un=71;e[e.uppercaseG=Un]="uppercaseG";let Hn=72;e[e.uppercaseH=Hn]="uppercaseH";let Vn=73;e[e.uppercaseI=Vn]="uppercaseI";let Wn=74;e[e.uppercaseJ=Wn]="uppercaseJ";let zn=75;e[e.uppercaseK=zn]="uppercaseK";let Gn=76;e[e.uppercaseL=Gn]="uppercaseL";let Xn=77;e[e.uppercaseM=Xn]="uppercaseM";let Jn=78;e[e.uppercaseN=Jn]="uppercaseN";let Kn=79;e[e.uppercaseO=Kn]="uppercaseO";let Qn=80;e[e.uppercaseP=Qn]="uppercaseP";let Yn=81;e[e.uppercaseQ=Yn]="uppercaseQ";let Zn=82;e[e.uppercaseR=Zn]="uppercaseR";let er=83;e[e.uppercaseS=er]="uppercaseS";let tr=84;e[e.uppercaseT=tr]="uppercaseT";let nr=85;e[e.uppercaseU=nr]="uppercaseU";let rr=86;e[e.uppercaseV=rr]="uppercaseV";let sr=87;e[e.uppercaseW=sr]="uppercaseW";let or=88;e[e.uppercaseX=or]="uppercaseX";let ir=89;e[e.uppercaseY=ir]="uppercaseY";let ar=90;e[e.uppercaseZ=ar]="uppercaseZ";let cr=91;e[e.leftSquareBracket=cr]="leftSquareBracket";let lr=92;e[e.backslash=lr]="backslash";let ur=93;e[e.rightSquareBracket=ur]="rightSquareBracket";let pr=94;e[e.caret=pr]="caret";let fr=95;e[e.underscore=fr]="underscore";let hr=96;e[e.graveAccent=hr]="graveAccent";let mr=97;e[e.lowercaseA=mr]="lowercaseA";let dr=98;e[e.lowercaseB=dr]="lowercaseB";let kr=99;e[e.lowercaseC=kr]="lowercaseC";let gr=100;e[e.lowercaseD=gr]="lowercaseD";let xr=101;e[e.lowercaseE=xr]="lowercaseE";let yr=102;e[e.lowercaseF=yr]="lowercaseF";let _r=103;e[e.lowercaseG=_r]="lowercaseG";let br=104;e[e.lowercaseH=br]="lowercaseH";let Tr=105;e[e.lowercaseI=Tr]="lowercaseI";let Ir=106;e[e.lowercaseJ=Ir]="lowercaseJ";let wr=107;e[e.lowercaseK=wr]="lowercaseK";let vr=108;e[e.lowercaseL=vr]="lowercaseL";let Sr=109;e[e.lowercaseM=Sr]="lowercaseM";let Er=110;e[e.lowercaseN=Er]="lowercaseN";let Ar=111;e[e.lowercaseO=Ar]="lowercaseO";let Pr=112;e[e.lowercaseP=Pr]="lowercaseP";let Cr=113;e[e.lowercaseQ=Cr]="lowercaseQ";let Nr=114;e[e.lowercaseR=Nr]="lowercaseR";let Rr=115;e[e.lowercaseS=Rr]="lowercaseS";let Lr=116;e[e.lowercaseT=Lr]="lowercaseT";let Dr=117;e[e.lowercaseU=Dr]="lowercaseU";let Or=118;e[e.lowercaseV=Or]="lowercaseV";let jr=119;e[e.lowercaseW=jr]="lowercaseW";let Fr=120;e[e.lowercaseX=Fr]="lowercaseX";let Mr=121;e[e.lowercaseY=Mr]="lowercaseY";let Br=122;e[e.lowercaseZ=Br]="lowercaseZ";let qr=123;e[e.leftCurlyBrace=qr]="leftCurlyBrace";let $r=124;e[e.verticalBar=$r]="verticalBar";let Ur=125;e[e.rightCurlyBrace=Ur]="rightCurlyBrace";let Hr=126;e[e.tilde=Hr]="tilde";let Vr=160;e[e.nonBreakingSpace=Vr]="nonBreakingSpace";let Wr=5760;e[e.oghamSpaceMark=Wr]="oghamSpaceMark";let zr=8232;e[e.lineSeparator=zr]="lineSeparator";let Gr=8233;e[e.paragraphSeparator=Gr]="paragraphSeparator"})(u||(u={}));var rt,R,L,s,g,fo;function Ne(){return fo++}function ho(e){if("pos"in e){let n=ca(e.pos);e.message+=` (${n.line}:${n.column})`,e.loc=n}return e}var Qr=class{constructor(n,r){this.line=n,this.column=r}};function ca(e){let n=1,r=1;for(let o=0;o<e;o++)g.charCodeAt(o)===u.lineFeed?(n++,r=1):r++;return new Qr(n,r)}function mo(e,n,r,o){g=e,s=new kt,fo=1,rt=n,R=r,L=o}function _(e){return s.contextualKeyword===e}function st(e){let n=xe();return n.type===t.name&&n.contextualKeyword===e}function W(e){return s.contextualKeyword===e&&f(t.name)}function U(e){W(e)||S()}function J(){return a(t.eof)||a(t.braceR)||X()}function X(){let e=s.tokens[s.tokens.length-1],n=e?e.end:0;for(let r=n;r<s.start;r++){let o=g.charCodeAt(r);if(o===u.lineFeed||o===u.carriageReturn||o===8232||o===8233)return!0}return!1}function tn(){let e=gt();for(let n=s.end;n<e;n++){let r=g.charCodeAt(n);if(r===u.lineFeed||r===u.carriageReturn||r===8232||r===8233)return!0}return!1}function ae(){return f(t.semi)||J()}function M(){ae()||S('Unexpected token, expected ";"')}function m(e){f(e)||S(`Unexpected token, expected "${Jr(e)}"`)}function S(e="Unexpected token",n=s.start){if(s.error)return;let r=new SyntaxError(e);r.pos=n,s.error=r,s.pos=g.length,P(t.eof)}var Yr=[9,11,12,u.space,u.nonBreakingSpace,u.oghamSpaceMark,8192,8193,8194,8195,8196,8197,8198,8199,8200,8201,8202,8239,8287,12288,65279],Zr=/(?:\s|\/\/.*|\/\*[^]*?\*\/)*/g,es=new Uint8Array(65536);for(let e of Yr)es[e]=1;function la(e){if(e<48)return e===36;if(e<58)return!0;if(e<65)return!1;if(e<91)return!0;if(e<97)return e===95;if(e<123)return!0;if(e<128)return!1;throw new Error("Should not be called with non-ASCII char code.")}var Y=new Uint8Array(65536);for(let e=0;e<128;e++)Y[e]=la(e)?1:0;for(let e=128;e<65536;e++)Y[e]=1;for(let e of Yr)Y[e]=0;Y[8232]=0;Y[8233]=0;var ye=Y.slice();for(let e=u.digit0;e<=u.digit9;e++)ye[e]=0;var ts=new Int32Array([-1,27,783,918,1755,2376,2862,3483,-1,3699,-1,4617,4752,4833,5130,5508,5940,-1,6480,6939,7749,8181,8451,8613,-1,8829,-1,-1,-1,54,243,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,432,-1,-1,-1,675,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,81,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,108,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,135,-1,-1,-1,-1,-1,-1,-1,-1,-1,162,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,189,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,216,-1,-1,-1,-1,-1,-1,l._abstract<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,270,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,297,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,324,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,351,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,378,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,405,-1,-1,-1,-1,-1,-1,-1,-1,l._accessor<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._as<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,459,-1,-1,-1,-1,-1,594,-1,-1,-1,-1,-1,-1,486,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,513,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,540,-1,-1,-1,-1,-1,-1,l._assert<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,567,-1,-1,-1,-1,-1,-1,-1,l._asserts<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,621,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,648,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._async<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,702,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,729,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,756,-1,-1,-1,-1,-1,-1,l._await<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,810,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,837,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,864,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,891,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._break<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,945,-1,-1,-1,-1,-1,-1,1107,-1,-1,-1,1242,-1,-1,1350,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,972,1026,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,999,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._case<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1053,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1080,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._catch<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1134,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1161,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1188,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1215,-1,-1,-1,-1,-1,-1,-1,l._checks<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1269,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1296,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1323,-1,-1,-1,-1,-1,-1,-1,(t._class<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1377,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1404,1620,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1431,-1,-1,-1,-1,-1,-1,(t._const<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1458,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1485,-1,-1,-1,-1,-1,-1,-1,-1,1512,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1539,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1566,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1593,-1,-1,-1,-1,-1,-1,-1,-1,l._constructor<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1647,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1674,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1701,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1728,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._continue<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1782,-1,-1,-1,-1,-1,-1,-1,-1,-1,2349,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1809,1971,-1,-1,2106,-1,-1,-1,-1,-1,2241,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1836,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1863,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1890,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1917,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1944,-1,-1,-1,-1,-1,-1,-1,-1,(t._debugger<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1998,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2025,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2052,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2079,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._declare<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2133,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2160,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2187,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2214,-1,-1,-1,-1,-1,-1,(t._default<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2268,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2295,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2322,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._delete<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._do<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2403,-1,2484,-1,-1,-1,-1,-1,-1,-1,-1,-1,2565,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2430,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2457,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._else<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2511,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2538,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._enum<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2592,-1,-1,-1,2727,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2619,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2646,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2673,-1,-1,-1,-1,-1,-1,(t._export<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2700,-1,-1,-1,-1,-1,-1,-1,l._exports<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2754,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2781,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2808,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2835,-1,-1,-1,-1,-1,-1,-1,(t._extends<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2889,-1,-1,-1,-1,-1,-1,-1,2997,-1,-1,-1,-1,-1,3159,-1,-1,3213,-1,-1,3294,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2916,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2943,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2970,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._false<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3024,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3051,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3078,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3105,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3132,-1,(t._finally<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3186,-1,-1,-1,-1,-1,-1,-1,-1,(t._for<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3240,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3267,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._from<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3321,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3348,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3375,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3402,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3429,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3456,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._function<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3510,-1,-1,-1,-1,-1,-1,3564,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3537,-1,-1,-1,-1,-1,-1,l._get<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3591,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3618,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3645,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3672,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._global<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3726,-1,-1,-1,-1,-1,-1,3753,4077,-1,-1,-1,-1,4590,-1,-1,-1,-1,-1,-1,-1,(t._if<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3780,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3807,-1,-1,3996,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3834,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3861,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3888,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3915,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3942,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3969,-1,-1,-1,-1,-1,-1,-1,l._implements<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4023,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4050,-1,-1,-1,-1,-1,-1,(t._import<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._in<<1)+1,-1,-1,-1,-1,-1,4104,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4185,4401,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4131,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4158,-1,-1,-1,-1,-1,-1,-1,-1,l._infer<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4212,-1,-1,-1,-1,-1,-1,-1,4239,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4266,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4293,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4320,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4347,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4374,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._instanceof<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4428,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4455,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4482,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4509,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4536,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4563,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._interface<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._is<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4644,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4671,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4698,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4725,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._keyof<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4779,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4806,-1,-1,-1,-1,-1,-1,(t._let<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4860,-1,-1,-1,-1,-1,4995,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4887,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4914,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4941,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4968,-1,-1,-1,-1,-1,-1,-1,l._mixins<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5022,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5049,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5076,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5103,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._module<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5157,-1,-1,-1,5373,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5427,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5184,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5211,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5238,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5265,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5292,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5319,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5346,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._namespace<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5400,-1,-1,-1,(t._new<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5454,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5481,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._null<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5535,-1,-1,-1,-1,-1,-1,-1,-1,-1,5562,-1,-1,-1,-1,5697,5751,-1,-1,-1,-1,l._of<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5589,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5616,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5643,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5670,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._opaque<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5724,-1,-1,-1,-1,-1,-1,l._out<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5778,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5805,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5832,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5859,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5886,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5913,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._override<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5967,-1,-1,6345,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5994,-1,-1,-1,-1,-1,6129,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6021,-1,-1,-1,-1,-1,6048,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6075,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6102,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._private<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6156,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6183,-1,-1,-1,-1,-1,-1,-1,-1,-1,6318,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6210,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6237,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6264,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6291,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._protected<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._proto<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6372,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6399,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6426,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6453,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._public<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6507,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6534,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6696,-1,-1,6831,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6561,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6588,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6615,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6642,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6669,-1,l._readonly<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6723,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6750,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6777,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6804,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._require<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6858,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6885,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6912,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._return<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6966,-1,-1,-1,7182,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7236,7371,-1,7479,-1,7614,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6993,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7020,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7047,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7074,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7101,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7128,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7155,-1,-1,-1,-1,-1,-1,-1,l._satisfies<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7209,-1,-1,-1,-1,-1,-1,l._set<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7263,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7290,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7317,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7344,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._static<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7398,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7425,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7452,-1,-1,-1,-1,-1,-1,-1,-1,(t._super<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7506,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7533,-1,-1,-1,-1,-1,-1,-1,-1,-1,7560,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7587,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._switch<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7641,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7668,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7695,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7722,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._symbol<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7776,-1,-1,-1,-1,-1,-1,-1,-1,-1,7938,-1,-1,-1,-1,-1,-1,8046,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7803,-1,-1,-1,-1,-1,-1,-1,-1,7857,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7830,-1,-1,-1,-1,-1,-1,-1,(t._this<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7884,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7911,-1,-1,-1,(t._throw<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7965,-1,-1,-1,8019,-1,-1,-1,-1,-1,-1,7992,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._true<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._try<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8073,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8100,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._type<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8127,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8154,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._typeof<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8208,-1,-1,-1,-1,8343,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8235,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8262,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8289,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8316,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._unique<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8370,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8397,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8424,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._using<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8478,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8532,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8505,-1,-1,-1,-1,-1,-1,-1,-1,(t._var<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8559,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8586,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._void<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8640,8748,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8667,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8694,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8721,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._while<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8775,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8802,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._with<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8856,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8883,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8910,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8937,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._yield<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1]);function ns(){let e=0,n=0,r=s.pos;for(;r<g.length&&(n=g.charCodeAt(r),!(n<u.lowercaseA||n>u.lowercaseZ));){let i=ts[e+(n-u.lowercaseA)+1];if(i===-1)break;e=i,r++}let o=ts[e];if(o>-1&&!Y[n]){s.pos=r,o&1?P(o>>>1):P(t.name,o>>>1);return}for(;r<g.length;){let i=g.charCodeAt(r);if(Y[i])r++;else if(i===u.backslash){if(r+=2,g.charCodeAt(r)===u.leftCurlyBrace){for(;r<g.length&&g.charCodeAt(r)!==u.rightCurlyBrace;)r++;r++}}else if(i===u.atSign&&g.charCodeAt(r+1)===u.atSign)r+=2;else break}s.pos=r,P(t.name)}var b;(function(e){e[e.Access=0]="Access";let r=1;e[e.ExportAccess=r]="ExportAccess";let o=r+1;e[e.TopLevelDeclaration=o]="TopLevelDeclaration";let i=o+1;e[e.FunctionScopedDeclaration=i]="FunctionScopedDeclaration";let c=i+1;e[e.BlockScopedDeclaration=c]="BlockScopedDeclaration";let p=c+1;e[e.ObjectShorthandTopLevelDeclaration=p]="ObjectShorthandTopLevelDeclaration";let h=p+1;e[e.ObjectShorthandFunctionScopedDeclaration=h]="ObjectShorthandFunctionScopedDeclaration";let x=h+1;e[e.ObjectShorthandBlockScopedDeclaration=x]="ObjectShorthandBlockScopedDeclaration";let k=x+1;e[e.ObjectShorthand=k]="ObjectShorthand";let y=k+1;e[e.ImportDeclaration=y]="ImportDeclaration";let w=y+1;e[e.ObjectKey=w]="ObjectKey";let I=w+1;e[e.ImportAccess=I]="ImportAccess"})(b||(b={}));var re;(function(e){e[e.NoChildren=0]="NoChildren";let r=1;e[e.OneChild=r]="OneChild";let o=r+1;e[e.StaticChildren=o]="StaticChildren";let i=o+1;e[e.KeyAfterPropSpread=i]="KeyAfterPropSpread"})(re||(re={}));function nn(e){let n=e.identifierRole;return n===b.TopLevelDeclaration||n===b.FunctionScopedDeclaration||n===b.BlockScopedDeclaration||n===b.ObjectShorthandTopLevelDeclaration||n===b.ObjectShorthandFunctionScopedDeclaration||n===b.ObjectShorthandBlockScopedDeclaration}function ko(e){let n=e.identifierRole;return n===b.FunctionScopedDeclaration||n===b.BlockScopedDeclaration||n===b.ObjectShorthandFunctionScopedDeclaration||n===b.ObjectShorthandBlockScopedDeclaration}function rn(e){let n=e.identifierRole;return n===b.TopLevelDeclaration||n===b.ObjectShorthandTopLevelDeclaration||n===b.ImportDeclaration}function go(e){let n=e.identifierRole;return n===b.TopLevelDeclaration||n===b.BlockScopedDeclaration||n===b.ObjectShorthandTopLevelDeclaration||n===b.ObjectShorthandBlockScopedDeclaration}function xo(e){let n=e.identifierRole;return n===b.FunctionScopedDeclaration||n===b.ObjectShorthandFunctionScopedDeclaration}function yo(e){return e.identifierRole===b.ObjectShorthandTopLevelDeclaration||e.identifierRole===b.ObjectShorthandBlockScopedDeclaration||e.identifierRole===b.ObjectShorthandFunctionScopedDeclaration}var Re=class{constructor(){this.type=s.type,this.contextualKeyword=s.contextualKeyword,this.start=s.start,this.end=s.end,this.scopeDepth=s.scopeDepth,this.isType=s.isType,this.identifierRole=null,this.jsxRole=null,this.shadowsGlobal=!1,this.isAsyncOperation=!1,this.contextId=null,this.rhsEndIndex=null,this.isExpression=!1,this.numNullishCoalesceStarts=0,this.numNullishCoalesceEnds=0,this.isOptionalChainStart=!1,this.isOptionalChainEnd=!1,this.subscriptStartIndex=null,this.nullishStartIndex=null}};function d(){s.tokens.push(new Re),as()}function he(){s.tokens.push(new Re),s.start=s.pos,wa()}function _o(){s.type===t.assign&&--s.pos,ba()}function N(e){for(let r=s.tokens.length-e;r<s.tokens.length;r++)s.tokens[r].isType=!0;let n=s.isType;return s.isType=!0,n}function C(e){s.isType=e}function f(e){return a(e)?(d(),!0):!1}function sn(e){let n=s.isType;s.isType=!0,f(e),s.isType=n}function a(e){return s.type===e}function B(){let e=s.snapshot();d();let n=s.type;return s.restoreFromSnapshot(e),n}var ss=class{constructor(n,r){this.type=n,this.contextualKeyword=r}};function xe(){let e=s.snapshot();d();let n=s.type,r=s.contextualKeyword;return s.restoreFromSnapshot(e),new ss(n,r)}function gt(){return os(s.pos)}function os(e){Zr.lastIndex=e;let n=Zr.exec(g);return e+n[0].length}function is(){return g.charCodeAt(gt())}function as(){if(ls(),s.start=s.pos,s.pos>=g.length){let e=s.tokens;e.length>=2&&e[e.length-1].start>=g.length&&e[e.length-2].start>=g.length&&S("Unexpectedly reached the end of input."),P(t.eof);return}ua(g.charCodeAt(s.pos))}function ua(e){ye[e]||e===u.backslash||e===u.atSign&&g.charCodeAt(s.pos+1)===u.atSign?ns():us(e)}function pa(){for(;g.charCodeAt(s.pos)!==u.asterisk||g.charCodeAt(s.pos+1)!==u.slash;)if(s.pos++,s.pos>g.length){S("Unterminated comment",s.pos-2);return}s.pos+=2}function cs(e){let n=g.charCodeAt(s.pos+=e);if(s.pos<g.length)for(;n!==u.lineFeed&&n!==u.carriageReturn&&n!==u.lineSeparator&&n!==u.paragraphSeparator&&++s.pos<g.length;)n=g.charCodeAt(s.pos)}function ls(){for(;s.pos<g.length;){let e=g.charCodeAt(s.pos);switch(e){case u.carriageReturn:g.charCodeAt(s.pos+1)===u.lineFeed&&++s.pos;case u.lineFeed:case u.lineSeparator:case u.paragraphSeparator:++s.pos;break;case u.slash:switch(g.charCodeAt(s.pos+1)){case u.asterisk:s.pos+=2,pa();break;case u.slash:cs(2);break;default:return}break;default:if(es[e])++s.pos;else return}}}function P(e,n=l.NONE){s.end=s.pos,s.type=e,s.contextualKeyword=n}function fa(){let e=g.charCodeAt(s.pos+1);if(e>=u.digit0&&e<=u.digit9){To(!0);return}e===u.dot&&g.charCodeAt(s.pos+2)===u.dot?(s.pos+=3,P(t.ellipsis)):(++s.pos,P(t.dot))}function ha(){g.charCodeAt(s.pos+1)===u.equalsTo?j(t.assign,2):j(t.slash,1)}function ma(e){let n=e===u.asterisk?t.star:t.modulo,r=1,o=g.charCodeAt(s.pos+1);e===u.asterisk&&o===u.asterisk&&(r++,o=g.charCodeAt(s.pos+2),n=t.exponent),o===u.equalsTo&&g.charCodeAt(s.pos+2)!==u.greaterThan&&(r++,n=t.assign),j(n,r)}function da(e){let n=g.charCodeAt(s.pos+1);if(n===e){g.charCodeAt(s.pos+2)===u.equalsTo?j(t.assign,3):j(e===u.verticalBar?t.logicalOR:t.logicalAND,2);return}if(e===u.verticalBar){if(n===u.greaterThan){j(t.pipeline,2);return}else if(n===u.rightCurlyBrace&&L){j(t.braceBarR,2);return}}if(n===u.equalsTo){j(t.assign,2);return}j(e===u.verticalBar?t.bitwiseOR:t.bitwiseAND,1)}function ka(){g.charCodeAt(s.pos+1)===u.equalsTo?j(t.assign,2):j(t.bitwiseXOR,1)}function ga(e){let n=g.charCodeAt(s.pos+1);if(n===e){j(t.preIncDec,2);return}n===u.equalsTo?j(t.assign,2):e===u.plusSign?j(t.plus,1):j(t.minus,1)}function xa(){let e=g.charCodeAt(s.pos+1);if(e===u.lessThan){if(g.charCodeAt(s.pos+2)===u.equalsTo){j(t.assign,3);return}s.isType?j(t.lessThan,1):j(t.bitShiftL,2);return}e===u.equalsTo?j(t.relationalOrEqual,2):j(t.lessThan,1)}function bo(){if(s.isType){j(t.greaterThan,1);return}let e=g.charCodeAt(s.pos+1);if(e===u.greaterThan){let n=g.charCodeAt(s.pos+2)===u.greaterThan?3:2;if(g.charCodeAt(s.pos+n)===u.equalsTo){j(t.assign,n+1);return}j(t.bitShiftR,n);return}e===u.equalsTo?j(t.relationalOrEqual,2):j(t.greaterThan,1)}function on(){s.type===t.greaterThan&&(s.pos-=1,bo())}function ya(e){let n=g.charCodeAt(s.pos+1);if(n===u.equalsTo){j(t.equality,g.charCodeAt(s.pos+2)===u.equalsTo?3:2);return}if(e===u.equalsTo&&n===u.greaterThan){s.pos+=2,P(t.arrow);return}j(e===u.equalsTo?t.eq:t.bang,1)}function _a(){let e=g.charCodeAt(s.pos+1),n=g.charCodeAt(s.pos+2);e===u.questionMark&&!(L&&s.isType)?n===u.equalsTo?j(t.assign,3):j(t.nullishCoalescing,2):e===u.dot&&!(n>=u.digit0&&n<=u.digit9)?(s.pos+=2,P(t.questionDot)):(++s.pos,P(t.question))}function us(e){switch(e){case u.numberSign:++s.pos,P(t.hash);return;case u.dot:fa();return;case u.leftParenthesis:++s.pos,P(t.parenL);return;case u.rightParenthesis:++s.pos,P(t.parenR);return;case u.semicolon:++s.pos,P(t.semi);return;case u.comma:++s.pos,P(t.comma);return;case u.leftSquareBracket:++s.pos,P(t.bracketL);return;case u.rightSquareBracket:++s.pos,P(t.bracketR);return;case u.leftCurlyBrace:L&&g.charCodeAt(s.pos+1)===u.verticalBar?j(t.braceBarL,2):(++s.pos,P(t.braceL));return;case u.rightCurlyBrace:++s.pos,P(t.braceR);return;case u.colon:g.charCodeAt(s.pos+1)===u.colon?j(t.doubleColon,2):(++s.pos,P(t.colon));return;case u.questionMark:_a();return;case u.atSign:++s.pos,P(t.at);return;case u.graveAccent:++s.pos,P(t.backQuote);return;case u.digit0:{let n=g.charCodeAt(s.pos+1);if(n===u.lowercaseX||n===u.uppercaseX||n===u.lowercaseO||n===u.uppercaseO||n===u.lowercaseB||n===u.uppercaseB){Ta();return}}case u.digit1:case u.digit2:case u.digit3:case u.digit4:case u.digit5:case u.digit6:case u.digit7:case u.digit8:case u.digit9:To(!1);return;case u.quotationMark:case u.apostrophe:Ia(e);return;case u.slash:ha();return;case u.percentSign:case u.asterisk:ma(e);return;case u.verticalBar:case u.ampersand:da(e);return;case u.caret:ka();return;case u.plusSign:case u.dash:ga(e);return;case u.lessThan:xa();return;case u.greaterThan:bo();return;case u.equalsTo:case u.exclamationMark:ya(e);return;case u.tilde:j(t.tilde,1);return;default:break}S(`Unexpected character '${String.fromCharCode(e)}'`,s.pos)}function j(e,n){s.pos+=n,P(e)}function ba(){let e=s.pos,n=!1,r=!1;for(;;){if(s.pos>=g.length){S("Unterminated regular expression",e);return}let o=g.charCodeAt(s.pos);if(n)n=!1;else{if(o===u.leftSquareBracket)r=!0;else if(o===u.rightSquareBracket&&r)r=!1;else if(o===u.slash&&!r)break;n=o===u.backslash}++s.pos}++s.pos,va(),P(t.regexp)}function rs(){for(;;){let e=g.charCodeAt(s.pos);if(e>=u.digit0&&e<=u.digit9||e===u.underscore)s.pos++;else break}}function Ta(){for(s.pos+=2;;){let n=g.charCodeAt(s.pos);if(n>=u.digit0&&n<=u.digit9||n>=u.lowercaseA&&n<=u.lowercaseF||n>=u.uppercaseA&&n<=u.uppercaseF||n===u.underscore)s.pos++;else break}g.charCodeAt(s.pos)===u.lowercaseN?(++s.pos,P(t.bigint)):P(t.num)}function To(e){let n=!1,r=!1;e||rs();let o=g.charCodeAt(s.pos);if(o===u.dot&&(++s.pos,rs(),o=g.charCodeAt(s.pos)),(o===u.uppercaseE||o===u.lowercaseE)&&(o=g.charCodeAt(++s.pos),(o===u.plusSign||o===u.dash)&&++s.pos,rs(),o=g.charCodeAt(s.pos)),o===u.lowercaseN?(++s.pos,n=!0):o===u.lowercaseM&&(++s.pos,r=!0),n){P(t.bigint);return}if(r){P(t.decimal);return}P(t.num)}function Ia(e){for(s.pos++;;){if(s.pos>=g.length){S("Unterminated string constant");return}let n=g.charCodeAt(s.pos);if(n===u.backslash)s.pos++;else if(n===e)break;s.pos++}s.pos++,P(t.string)}function wa(){for(;;){if(s.pos>=g.length){S("Unterminated template");return}let e=g.charCodeAt(s.pos);if(e===u.graveAccent||e===u.dollarSign&&g.charCodeAt(s.pos+1)===u.leftCurlyBrace){if(s.pos===s.start&&a(t.template))if(e===u.dollarSign){s.pos+=2,P(t.dollarBraceL);return}else{++s.pos,P(t.backQuote);return}P(t.template);return}e===u.backslash&&s.pos++,s.pos++}}function va(){for(;s.pos<g.length;){let e=g.charCodeAt(s.pos);if(Y[e])s.pos++;else if(e===u.backslash){if(s.pos+=2,g.charCodeAt(s.pos)===u.leftCurlyBrace){for(;s.pos<g.length&&g.charCodeAt(s.pos)!==u.rightCurlyBrace;)s.pos++;s.pos++}}else break}}function me(e,n=e.currentIndex()){let r=n+1;if(an(e,r)){let o=e.identifierNameAtIndex(n);return{isType:!1,leftName:o,rightName:o,endIndex:r}}if(r++,an(e,r))return{isType:!0,leftName:null,rightName:null,endIndex:r};if(r++,an(e,r))return{isType:!1,leftName:e.identifierNameAtIndex(n),rightName:e.identifierNameAtIndex(n+2),endIndex:r};if(r++,an(e,r))return{isType:!0,leftName:null,rightName:null,endIndex:r};throw new Error(`Unexpected import/export specifier at ${n}`)}function an(e,n){let r=e.tokens[n];return r.type===t.braceR||r.type===t.comma}var Io=new Map([["quot",'"'],["amp","&"],["apos","'"],["lt","<"],["gt",">"],["nbsp","\xA0"],["iexcl","\xA1"],["cent","\xA2"],["pound","\xA3"],["curren","\xA4"],["yen","\xA5"],["brvbar","\xA6"],["sect","\xA7"],["uml","\xA8"],["copy","\xA9"],["ordf","\xAA"],["laquo","\xAB"],["not","\xAC"],["shy","\xAD"],["reg","\xAE"],["macr","\xAF"],["deg","\xB0"],["plusmn","\xB1"],["sup2","\xB2"],["sup3","\xB3"],["acute","\xB4"],["micro","\xB5"],["para","\xB6"],["middot","\xB7"],["cedil","\xB8"],["sup1","\xB9"],["ordm","\xBA"],["raquo","\xBB"],["frac14","\xBC"],["frac12","\xBD"],["frac34","\xBE"],["iquest","\xBF"],["Agrave","\xC0"],["Aacute","\xC1"],["Acirc","\xC2"],["Atilde","\xC3"],["Auml","\xC4"],["Aring","\xC5"],["AElig","\xC6"],["Ccedil","\xC7"],["Egrave","\xC8"],["Eacute","\xC9"],["Ecirc","\xCA"],["Euml","\xCB"],["Igrave","\xCC"],["Iacute","\xCD"],["Icirc","\xCE"],["Iuml","\xCF"],["ETH","\xD0"],["Ntilde","\xD1"],["Ograve","\xD2"],["Oacute","\xD3"],["Ocirc","\xD4"],["Otilde","\xD5"],["Ouml","\xD6"],["times","\xD7"],["Oslash","\xD8"],["Ugrave","\xD9"],["Uacute","\xDA"],["Ucirc","\xDB"],["Uuml","\xDC"],["Yacute","\xDD"],["THORN","\xDE"],["szlig","\xDF"],["agrave","\xE0"],["aacute","\xE1"],["acirc","\xE2"],["atilde","\xE3"],["auml","\xE4"],["aring","\xE5"],["aelig","\xE6"],["ccedil","\xE7"],["egrave","\xE8"],["eacute","\xE9"],["ecirc","\xEA"],["euml","\xEB"],["igrave","\xEC"],["iacute","\xED"],["icirc","\xEE"],["iuml","\xEF"],["eth","\xF0"],["ntilde","\xF1"],["ograve","\xF2"],["oacute","\xF3"],["ocirc","\xF4"],["otilde","\xF5"],["ouml","\xF6"],["divide","\xF7"],["oslash","\xF8"],["ugrave","\xF9"],["uacute","\xFA"],["ucirc","\xFB"],["uuml","\xFC"],["yacute","\xFD"],["thorn","\xFE"],["yuml","\xFF"],["OElig","\u0152"],["oelig","\u0153"],["Scaron","\u0160"],["scaron","\u0161"],["Yuml","\u0178"],["fnof","\u0192"],["circ","\u02C6"],["tilde","\u02DC"],["Alpha","\u0391"],["Beta","\u0392"],["Gamma","\u0393"],["Delta","\u0394"],["Epsilon","\u0395"],["Zeta","\u0396"],["Eta","\u0397"],["Theta","\u0398"],["Iota","\u0399"],["Kappa","\u039A"],["Lambda","\u039B"],["Mu","\u039C"],["Nu","\u039D"],["Xi","\u039E"],["Omicron","\u039F"],["Pi","\u03A0"],["Rho","\u03A1"],["Sigma","\u03A3"],["Tau","\u03A4"],["Upsilon","\u03A5"],["Phi","\u03A6"],["Chi","\u03A7"],["Psi","\u03A8"],["Omega","\u03A9"],["alpha","\u03B1"],["beta","\u03B2"],["gamma","\u03B3"],["delta","\u03B4"],["epsilon","\u03B5"],["zeta","\u03B6"],["eta","\u03B7"],["theta","\u03B8"],["iota","\u03B9"],["kappa","\u03BA"],["lambda","\u03BB"],["mu","\u03BC"],["nu","\u03BD"],["xi","\u03BE"],["omicron","\u03BF"],["pi","\u03C0"],["rho","\u03C1"],["sigmaf","\u03C2"],["sigma","\u03C3"],["tau","\u03C4"],["upsilon","\u03C5"],["phi","\u03C6"],["chi","\u03C7"],["psi","\u03C8"],["omega","\u03C9"],["thetasym","\u03D1"],["upsih","\u03D2"],["piv","\u03D6"],["ensp","\u2002"],["emsp","\u2003"],["thinsp","\u2009"],["zwnj","\u200C"],["zwj","\u200D"],["lrm","\u200E"],["rlm","\u200F"],["ndash","\u2013"],["mdash","\u2014"],["lsquo","\u2018"],["rsquo","\u2019"],["sbquo","\u201A"],["ldquo","\u201C"],["rdquo","\u201D"],["bdquo","\u201E"],["dagger","\u2020"],["Dagger","\u2021"],["bull","\u2022"],["hellip","\u2026"],["permil","\u2030"],["prime","\u2032"],["Prime","\u2033"],["lsaquo","\u2039"],["rsaquo","\u203A"],["oline","\u203E"],["frasl","\u2044"],["euro","\u20AC"],["image","\u2111"],["weierp","\u2118"],["real","\u211C"],["trade","\u2122"],["alefsym","\u2135"],["larr","\u2190"],["uarr","\u2191"],["rarr","\u2192"],["darr","\u2193"],["harr","\u2194"],["crarr","\u21B5"],["lArr","\u21D0"],["uArr","\u21D1"],["rArr","\u21D2"],["dArr","\u21D3"],["hArr","\u21D4"],["forall","\u2200"],["part","\u2202"],["exist","\u2203"],["empty","\u2205"],["nabla","\u2207"],["isin","\u2208"],["notin","\u2209"],["ni","\u220B"],["prod","\u220F"],["sum","\u2211"],["minus","\u2212"],["lowast","\u2217"],["radic","\u221A"],["prop","\u221D"],["infin","\u221E"],["ang","\u2220"],["and","\u2227"],["or","\u2228"],["cap","\u2229"],["cup","\u222A"],["int","\u222B"],["there4","\u2234"],["sim","\u223C"],["cong","\u2245"],["asymp","\u2248"],["ne","\u2260"],["equiv","\u2261"],["le","\u2264"],["ge","\u2265"],["sub","\u2282"],["sup","\u2283"],["nsub","\u2284"],["sube","\u2286"],["supe","\u2287"],["oplus","\u2295"],["otimes","\u2297"],["perp","\u22A5"],["sdot","\u22C5"],["lceil","\u2308"],["rceil","\u2309"],["lfloor","\u230A"],["rfloor","\u230B"],["lang","\u2329"],["rang","\u232A"],["loz","\u25CA"],["spades","\u2660"],["clubs","\u2663"],["hearts","\u2665"],["diams","\u2666"]]);function xt(e){let[n,r]=wo(e.jsxPragma||"React.createElement"),[o,i]=wo(e.jsxFragmentPragma||"React.Fragment");return{base:n,suffix:r,fragmentBase:o,fragmentSuffix:i}}function wo(e){let n=e.indexOf(".");return n===-1&&(n=e.length),[e.slice(0,n),e.slice(n)]}var H=class{getPrefixCode(){return""}getHoistedCode(){return""}getSuffixCode(){return""}};var yt=class e extends H{__init(){this.lastLineNumber=1}__init2(){this.lastIndex=0}__init3(){this.filenameVarName=null}__init4(){this.esmAutomaticImportNameResolutions={}}__init5(){this.cjsAutomaticModuleNameResolutions={}}constructor(n,r,o,i,c){super(),this.rootTransformer=n,this.tokens=r,this.importProcessor=o,this.nameManager=i,this.options=c,e.prototype.__init.call(this),e.prototype.__init2.call(this),e.prototype.__init3.call(this),e.prototype.__init4.call(this),e.prototype.__init5.call(this),this.jsxPragmaInfo=xt(c),this.isAutomaticRuntime=c.jsxRuntime==="automatic",this.jsxImportSource=c.jsxImportSource||"react"}process(){return this.tokens.matches1(t.jsxTagStart)?(this.processJSXTag(),!0):!1}getPrefixCode(){let n="";if(this.filenameVarName&&(n+=`const ${this.filenameVarName} = ${JSON.stringify(this.options.filePath||"")};`),this.isAutomaticRuntime)if(this.importProcessor)for(let[r,o]of Object.entries(this.cjsAutomaticModuleNameResolutions))n+=`var ${o} = require("${r}");`;else{let{createElement:r,...o}=this.esmAutomaticImportNameResolutions;r&&(n+=`import {createElement as ${r}} from "${this.jsxImportSource}";`);let i=Object.entries(o).map(([c,p])=>`${c} as ${p}`).join(", ");if(i){let c=this.jsxImportSource+(this.options.production?"/jsx-runtime":"/jsx-dev-runtime");n+=`import {${i}} from "${c}";`}}return n}processJSXTag(){let{jsxRole:n,start:r}=this.tokens.currentToken(),o=this.options.production?null:this.getElementLocationCode(r);this.isAutomaticRuntime&&n!==re.KeyAfterPropSpread?this.transformTagToJSXFunc(o,n):this.transformTagToCreateElement(o)}getElementLocationCode(n){return`lineNumber: ${this.getLineNumberForIndex(n)}`}getLineNumberForIndex(n){let r=this.tokens.code;for(;this.lastIndex<n&&this.lastIndex<r.length;)r[this.lastIndex]===`
`&&this.lastLineNumber++,this.lastIndex++;return this.lastLineNumber}transformTagToJSXFunc(n,r){let o=r===re.StaticChildren;this.tokens.replaceToken(this.getJSXFuncInvocationCode(o));let i=null;if(this.tokens.matches1(t.jsxTagEnd))this.tokens.replaceToken(`${this.getFragmentCode()}, {`),this.processAutomaticChildrenAndEndProps(r);else{if(this.processTagIntro(),this.tokens.appendCode(", {"),i=this.processProps(!0),this.tokens.matches2(t.slash,t.jsxTagEnd))this.tokens.appendCode("}");else if(this.tokens.matches1(t.jsxTagEnd))this.tokens.removeToken(),this.processAutomaticChildrenAndEndProps(r);else throw new Error("Expected either /> or > at the end of the tag.");i&&this.tokens.appendCode(`, ${i}`)}for(this.options.production||(i===null&&this.tokens.appendCode(", void 0"),this.tokens.appendCode(`, ${o}, ${this.getDevSource(n)}, this`)),this.tokens.removeInitialToken();!this.tokens.matches1(t.jsxTagEnd);)this.tokens.removeToken();this.tokens.replaceToken(")")}transformTagToCreateElement(n){if(this.tokens.replaceToken(this.getCreateElementInvocationCode()),this.tokens.matches1(t.jsxTagEnd))this.tokens.replaceToken(`${this.getFragmentCode()}, null`),this.processChildren(!0);else if(this.processTagIntro(),this.processPropsObjectWithDevInfo(n),!this.tokens.matches2(t.slash,t.jsxTagEnd))if(this.tokens.matches1(t.jsxTagEnd))this.tokens.removeToken(),this.processChildren(!0);else throw new Error("Expected either /> or > at the end of the tag.");for(this.tokens.removeInitialToken();!this.tokens.matches1(t.jsxTagEnd);)this.tokens.removeToken();this.tokens.replaceToken(")")}getJSXFuncInvocationCode(n){return this.options.production?n?this.claimAutoImportedFuncInvocation("jsxs","/jsx-runtime"):this.claimAutoImportedFuncInvocation("jsx","/jsx-runtime"):this.claimAutoImportedFuncInvocation("jsxDEV","/jsx-dev-runtime")}getCreateElementInvocationCode(){if(this.isAutomaticRuntime)return this.claimAutoImportedFuncInvocation("createElement","");{let{jsxPragmaInfo:n}=this;return`${this.importProcessor&&this.importProcessor.getIdentifierReplacement(n.base)||n.base}${n.suffix}(`}}getFragmentCode(){if(this.isAutomaticRuntime)return this.claimAutoImportedName("Fragment",this.options.production?"/jsx-runtime":"/jsx-dev-runtime");{let{jsxPragmaInfo:n}=this;return(this.importProcessor&&this.importProcessor.getIdentifierReplacement(n.fragmentBase)||n.fragmentBase)+n.fragmentSuffix}}claimAutoImportedFuncInvocation(n,r){let o=this.claimAutoImportedName(n,r);return this.importProcessor?`${o}.call(void 0, `:`${o}(`}claimAutoImportedName(n,r){if(this.importProcessor){let o=this.jsxImportSource+r;return this.cjsAutomaticModuleNameResolutions[o]||(this.cjsAutomaticModuleNameResolutions[o]=this.importProcessor.getFreeIdentifierForPath(o)),`${this.cjsAutomaticModuleNameResolutions[o]}.${n}`}else return this.esmAutomaticImportNameResolutions[n]||(this.esmAutomaticImportNameResolutions[n]=this.nameManager.claimFreeName(`_${n}`)),this.esmAutomaticImportNameResolutions[n]}processTagIntro(){let n=this.tokens.currentIndex()+1;for(;this.tokens.tokens[n].isType||!this.tokens.matches2AtIndex(n-1,t.jsxName,t.jsxName)&&!this.tokens.matches2AtIndex(n-1,t.greaterThan,t.jsxName)&&!this.tokens.matches1AtIndex(n,t.braceL)&&!this.tokens.matches1AtIndex(n,t.jsxTagEnd)&&!this.tokens.matches2AtIndex(n,t.slash,t.jsxTagEnd);)n++;if(n===this.tokens.currentIndex()+1){let r=this.tokens.identifierName();ps(r)&&this.tokens.replaceToken(`'${r}'`)}for(;this.tokens.currentIndex()<n;)this.rootTransformer.processToken()}processPropsObjectWithDevInfo(n){let r=this.options.production?"":`__self: this, __source: ${this.getDevSource(n)}`;if(!this.tokens.matches1(t.jsxName)&&!this.tokens.matches1(t.braceL)){r?this.tokens.appendCode(`, {${r}}`):this.tokens.appendCode(", null");return}this.tokens.appendCode(", {"),this.processProps(!1),r?this.tokens.appendCode(` ${r}}`):this.tokens.appendCode("}")}processProps(n){let r=null;for(;;){if(this.tokens.matches2(t.jsxName,t.eq)){let o=this.tokens.identifierName();if(n&&o==="key"){r!==null&&this.tokens.appendCode(r.replace(/[^\n]/g,"")),this.tokens.removeToken(),this.tokens.removeToken();let i=this.tokens.snapshot();this.processPropValue(),r=this.tokens.dangerouslyGetAndRemoveCodeSinceSnapshot(i);continue}else this.processPropName(o),this.tokens.replaceToken(": "),this.processPropValue()}else if(this.tokens.matches1(t.jsxName)){let o=this.tokens.identifierName();this.processPropName(o),this.tokens.appendCode(": true")}else if(this.tokens.matches1(t.braceL))this.tokens.replaceToken(""),this.rootTransformer.processBalancedCode(),this.tokens.replaceToken("");else break;this.tokens.appendCode(",")}return r}processPropName(n){n.includes("-")?this.tokens.replaceToken(`'${n}'`):this.tokens.copyToken()}processPropValue(){this.tokens.matches1(t.braceL)?(this.tokens.replaceToken(""),this.rootTransformer.processBalancedCode(),this.tokens.replaceToken("")):this.tokens.matches1(t.jsxTagStart)?this.processJSXTag():this.processStringPropValue()}processStringPropValue(){let n=this.tokens.currentToken(),r=this.tokens.code.slice(n.start+1,n.end-1),o=vo(r),i=Ea(r);this.tokens.replaceToken(i+o)}processAutomaticChildrenAndEndProps(n){n===re.StaticChildren?(this.tokens.appendCode(" children: ["),this.processChildren(!1),this.tokens.appendCode("]}")):(n===re.OneChild&&this.tokens.appendCode(" children: "),this.processChildren(!1),this.tokens.appendCode("}"))}processChildren(n){let r=n;for(;;){if(this.tokens.matches2(t.jsxTagStart,t.slash))return;let o=!1;if(this.tokens.matches1(t.braceL))this.tokens.matches2(t.braceL,t.braceR)?(this.tokens.replaceToken(""),this.tokens.replaceToken("")):(this.tokens.replaceToken(r?", ":""),this.rootTransformer.processBalancedCode(),this.tokens.replaceToken(""),o=!0);else if(this.tokens.matches1(t.jsxTagStart))this.tokens.appendCode(r?", ":""),this.processJSXTag(),o=!0;else if(this.tokens.matches1(t.jsxText)||this.tokens.matches1(t.jsxEmptyText))o=this.processChildTextElement(r);else throw new Error("Unexpected token when processing JSX children.");o&&(r=!0)}}processChildTextElement(n){let r=this.tokens.currentToken(),o=this.tokens.code.slice(r.start,r.end),i=vo(o),c=Sa(o);return c==='""'?(this.tokens.replaceToken(i),!1):(this.tokens.replaceToken(`${n?", ":""}${c}${i}`),!0)}getDevSource(n){return`{fileName: ${this.getFilenameVarName()}, ${n}}`}getFilenameVarName(){return this.filenameVarName||(this.filenameVarName=this.nameManager.claimFreeName("_jsxFileName")),this.filenameVarName}};function ps(e){let n=e.charCodeAt(0);return n>=u.lowercaseA&&n<=u.lowercaseZ}function Sa(e){let n="",r="",o=!1,i=!1;for(let c=0;c<e.length;c++){let p=e[c];if(p===" "||p==="	"||p==="\r")o||(r+=p);else if(p===`
`)r="",o=!0;else{if(i&&o&&(n+=" "),n+=r,r="",p==="&"){let{entity:h,newI:x}=So(e,c+1);c=x-1,n+=h}else n+=p;i=!0,o=!1}}return o||(n+=r),JSON.stringify(n)}function vo(e){let n=0,r=0;for(let o of e)o===`
`?(n++,r=0):o===" "&&r++;return`
`.repeat(n)+" ".repeat(r)}function Ea(e){let n="";for(let r=0;r<e.length;r++){let o=e[r];if(o===`
`)if(/\s/.test(e[r+1]))for(n+=" ";r<e.length&&/\s/.test(e[r+1]);)r++;else n+=`
`;else if(o==="&"){let{entity:i,newI:c}=So(e,r+1);n+=i,r=c-1}else n+=o}return JSON.stringify(n)}function So(e,n){let r="",o=0,i,c=n;if(e[c]==="#"){let p=10;c++;let h;if(e[c]==="x")for(p=16,c++,h=c;c<e.length&&Pa(e.charCodeAt(c));)c++;else for(h=c;c<e.length&&Aa(e.charCodeAt(c));)c++;if(e[c]===";"){let x=e.slice(h,c);x&&(c++,i=String.fromCodePoint(parseInt(x,p)))}}else for(;c<e.length&&o++<10;){let p=e[c];if(c++,p===";"){i=Io.get(r);break}r+=p}return i?{entity:i,newI:c}:{entity:"&",newI:n}}function Aa(e){return e>=u.digit0&&e<=u.digit9}function Pa(e){return e>=u.digit0&&e<=u.digit9||e>=u.lowercaseA&&e<=u.lowercaseF||e>=u.uppercaseA&&e<=u.uppercaseF}function cn(e,n){let r=xt(n),o=new Set;for(let i=0;i<e.tokens.length;i++){let c=e.tokens[i];if(c.type===t.name&&!c.isType&&(c.identifierRole===b.Access||c.identifierRole===b.ObjectShorthand||c.identifierRole===b.ExportAccess)&&!c.shadowsGlobal&&o.add(e.identifierNameForToken(c)),c.type===t.jsxTagStart&&o.add(r.base),c.type===t.jsxTagStart&&i+1<e.tokens.length&&e.tokens[i+1].type===t.jsxTagEnd&&(o.add(r.base),o.add(r.fragmentBase)),c.type===t.jsxName&&c.identifierRole===b.Access){let p=e.identifierNameForToken(c);(!ps(p)||e.tokens[i+1].type===t.dot)&&o.add(e.identifierNameForToken(c))}}return o}var _t=class e{__init(){this.nonTypeIdentifiers=new Set}__init2(){this.importInfoByPath=new Map}__init3(){this.importsToReplace=new Map}__init4(){this.identifierReplacements=new Map}__init5(){this.exportBindingsByLocalName=new Map}constructor(n,r,o,i,c,p,h){this.nameManager=n,this.tokens=r,this.enableLegacyTypeScriptModuleInterop=o,this.options=i,this.isTypeScriptTransformEnabled=c,this.keepUnusedImports=p,this.helperManager=h,e.prototype.__init.call(this),e.prototype.__init2.call(this),e.prototype.__init3.call(this),e.prototype.__init4.call(this),e.prototype.__init5.call(this)}preprocessTokens(){for(let n=0;n<this.tokens.tokens.length;n++)this.tokens.matches1AtIndex(n,t._import)&&!this.tokens.matches3AtIndex(n,t._import,t.name,t.eq)&&this.preprocessImportAtIndex(n),this.tokens.matches1AtIndex(n,t._export)&&!this.tokens.matches2AtIndex(n,t._export,t.eq)&&this.preprocessExportAtIndex(n);this.generateImportReplacements()}pruneTypeOnlyImports(){this.nonTypeIdentifiers=cn(this.tokens,this.options);for(let[n,r]of this.importInfoByPath.entries()){if(r.hasBareImport||r.hasStarExport||r.exportStarNames.length>0||r.namedExports.length>0)continue;[...r.defaultNames,...r.wildcardNames,...r.namedImports.map(({localName:i})=>i)].every(i=>this.shouldAutomaticallyElideImportedName(i))&&this.importsToReplace.set(n,"")}}shouldAutomaticallyElideImportedName(n){return this.isTypeScriptTransformEnabled&&!this.keepUnusedImports&&!this.nonTypeIdentifiers.has(n)}generateImportReplacements(){for(let[n,r]of this.importInfoByPath.entries()){let{defaultNames:o,wildcardNames:i,namedImports:c,namedExports:p,exportStarNames:h,hasStarExport:x}=r;if(o.length===0&&i.length===0&&c.length===0&&p.length===0&&h.length===0&&!x){this.importsToReplace.set(n,`require('${n}');`);continue}let k=this.getFreeIdentifierForPath(n),y;this.enableLegacyTypeScriptModuleInterop?y=k:y=i.length>0?i[0]:this.getFreeIdentifierForPath(n);let w=`var ${k} = require('${n}');`;if(i.length>0)for(let I of i){let A=this.enableLegacyTypeScriptModuleInterop?k:`${this.helperManager.getHelperName("interopRequireWildcard")}(${k})`;w+=` var ${I} = ${A};`}else h.length>0&&y!==k?w+=` var ${y} = ${this.helperManager.getHelperName("interopRequireWildcard")}(${k});`:o.length>0&&y!==k&&(w+=` var ${y} = ${this.helperManager.getHelperName("interopRequireDefault")}(${k});`);for(let{importedName:I,localName:A}of p)w+=` ${this.helperManager.getHelperName("createNamedExportFrom")}(${k}, '${A}', '${I}');`;for(let I of h)w+=` exports.${I} = ${y};`;x&&(w+=` ${this.helperManager.getHelperName("createStarExport")}(${k});`),this.importsToReplace.set(n,w);for(let I of o)this.identifierReplacements.set(I,`${y}.default`);for(let{importedName:I,localName:A}of c)this.identifierReplacements.set(A,`${k}.${I}`)}}getFreeIdentifierForPath(n){let r=n.split("/"),i=r[r.length-1].replace(/\W/g,"");return this.nameManager.claimFreeName(`_${i}`)}preprocessImportAtIndex(n){let r=[],o=[],i=[];if(n++,(this.tokens.matchesContextualAtIndex(n,l._type)||this.tokens.matches1AtIndex(n,t._typeof))&&!this.tokens.matches1AtIndex(n+1,t.comma)&&!this.tokens.matchesContextualAtIndex(n+1,l._from)||this.tokens.matches1AtIndex(n,t.parenL))return;if(this.tokens.matches1AtIndex(n,t.name)&&(r.push(this.tokens.identifierNameAtIndex(n)),n++,this.tokens.matches1AtIndex(n,t.comma)&&n++),this.tokens.matches1AtIndex(n,t.star)&&(n+=2,o.push(this.tokens.identifierNameAtIndex(n)),n++),this.tokens.matches1AtIndex(n,t.braceL)){let h=this.getNamedImports(n+1);n=h.newIndex;for(let x of h.namedImports)x.importedName==="default"?r.push(x.localName):i.push(x)}if(this.tokens.matchesContextualAtIndex(n,l._from)&&n++,!this.tokens.matches1AtIndex(n,t.string))throw new Error("Expected string token at the end of import statement.");let c=this.tokens.stringValueAtIndex(n),p=this.getImportInfo(c);p.defaultNames.push(...r),p.wildcardNames.push(...o),p.namedImports.push(...i),r.length===0&&o.length===0&&i.length===0&&(p.hasBareImport=!0)}preprocessExportAtIndex(n){if(this.tokens.matches2AtIndex(n,t._export,t._var)||this.tokens.matches2AtIndex(n,t._export,t._let)||this.tokens.matches2AtIndex(n,t._export,t._const))this.preprocessVarExportAtIndex(n);else if(this.tokens.matches2AtIndex(n,t._export,t._function)||this.tokens.matches2AtIndex(n,t._export,t._class)){let r=this.tokens.identifierNameAtIndex(n+2);this.addExportBinding(r,r)}else if(this.tokens.matches3AtIndex(n,t._export,t.name,t._function)){let r=this.tokens.identifierNameAtIndex(n+3);this.addExportBinding(r,r)}else this.tokens.matches2AtIndex(n,t._export,t.braceL)?this.preprocessNamedExportAtIndex(n):this.tokens.matches2AtIndex(n,t._export,t.star)&&this.preprocessExportStarAtIndex(n)}preprocessVarExportAtIndex(n){let r=0;for(let o=n+2;;o++)if(this.tokens.matches1AtIndex(o,t.braceL)||this.tokens.matches1AtIndex(o,t.dollarBraceL)||this.tokens.matches1AtIndex(o,t.bracketL))r++;else if(this.tokens.matches1AtIndex(o,t.braceR)||this.tokens.matches1AtIndex(o,t.bracketR))r--;else{if(r===0&&!this.tokens.matches1AtIndex(o,t.name))break;if(this.tokens.matches1AtIndex(1,t.eq)){let i=this.tokens.currentToken().rhsEndIndex;if(i==null)throw new Error("Expected = token with an end index.");o=i-1}else{let i=this.tokens.tokens[o];if(nn(i)){let c=this.tokens.identifierNameAtIndex(o);this.identifierReplacements.set(c,`exports.${c}`)}}}}preprocessNamedExportAtIndex(n){n+=2;let{newIndex:r,namedImports:o}=this.getNamedImports(n);if(n=r,this.tokens.matchesContextualAtIndex(n,l._from))n++;else{for(let{importedName:p,localName:h}of o)this.addExportBinding(p,h);return}if(!this.tokens.matches1AtIndex(n,t.string))throw new Error("Expected string token at the end of import statement.");let i=this.tokens.stringValueAtIndex(n);this.getImportInfo(i).namedExports.push(...o)}preprocessExportStarAtIndex(n){let r=null;if(this.tokens.matches3AtIndex(n,t._export,t.star,t._as)?(n+=3,r=this.tokens.identifierNameAtIndex(n),n+=2):n+=3,!this.tokens.matches1AtIndex(n,t.string))throw new Error("Expected string token at the end of star export statement.");let o=this.tokens.stringValueAtIndex(n),i=this.getImportInfo(o);r!==null?i.exportStarNames.push(r):i.hasStarExport=!0}getNamedImports(n){let r=[];for(;;){if(this.tokens.matches1AtIndex(n,t.braceR)){n++;break}let o=me(this.tokens,n);if(n=o.endIndex,o.isType||r.push({importedName:o.leftName,localName:o.rightName}),this.tokens.matches2AtIndex(n,t.comma,t.braceR)){n+=2;break}else if(this.tokens.matches1AtIndex(n,t.braceR)){n++;break}else if(this.tokens.matches1AtIndex(n,t.comma))n++;else throw new Error(`Unexpected token: ${JSON.stringify(this.tokens.tokens[n])}`)}return{newIndex:n,namedImports:r}}getImportInfo(n){let r=this.importInfoByPath.get(n);if(r)return r;let o={defaultNames:[],wildcardNames:[],namedImports:[],namedExports:[],hasBareImport:!1,exportStarNames:[],hasStarExport:!1};return this.importInfoByPath.set(n,o),o}addExportBinding(n,r){this.exportBindingsByLocalName.has(n)||this.exportBindingsByLocalName.set(n,[]),this.exportBindingsByLocalName.get(n).push(r)}claimImportCode(n){let r=this.importsToReplace.get(n);return this.importsToReplace.set(n,""),r||""}getIdentifierReplacement(n){return this.identifierReplacements.get(n)||null}resolveExportBinding(n){let r=this.exportBindingsByLocalName.get(n);return!r||r.length===0?null:r.map(o=>`exports.${o}`).join(" = ")}getGlobalNames(){return new Set([...this.identifierReplacements.keys(),...this.exportBindingsByLocalName.keys()])}};var Ca=44,Na=59,Eo="ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/",Co=new Uint8Array(64),Ra=new Uint8Array(128);for(let e=0;e<Eo.length;e++){let n=Eo.charCodeAt(e);Co[e]=n,Ra[n]=e}function bt(e,n){do{let r=n&31;n>>>=5,n>0&&(r|=32),e.write(Co[r])}while(n>0)}function Tt(e){return e<0?-e<<1|1:e<<1}var Ao=1024*16,Po=typeof TextDecoder<"u"?new TextDecoder:typeof Buffer<"u"?{decode(e){return Buffer.from(e.buffer,e.byteOffset,e.byteLength).toString()}}:{decode(e){let n="";for(let r=0;r<e.length;r++)n+=String.fromCharCode(e[r]);return n}},La=class{constructor(){this.pos=0,this.out="",this.buffer=new Uint8Array(Ao)}write(e){let{buffer:n}=this;n[this.pos++]=e,this.pos===Ao&&(this.out+=Po.decode(n),this.pos=0)}flush(){let{buffer:e,out:n,pos:r}=this;return r>0?n+Po.decode(e.subarray(0,r)):n}};function fs(e){let n=new La,r=0,o=0,i=0,c=0;for(let p=0;p<e.length;p++){let h=e[p];if(p>0&&n.write(Na),h.length===0)continue;let x=0;for(let k=0;k<h.length;k++){let y=h[k];k>0&&n.write(Ca),bt(n,Tt(y[0]-x)),x=y[0],y.length!==1&&(bt(n,Tt(y[1]-r)),bt(n,Tt(y[2]-o)),bt(n,Tt(y[3]-i)),r=y[1],o=y[2],i=y[3],y.length!==4&&(bt(n,Tt(y[4]-c)),c=y[4]))}}return n.flush()}var hs=class{constructor(){this._indexes={__proto__:null},this.array=[]}};function Da(e,n){return e._indexes[n]}function No(e,n){let r=Da(e,n);if(r!==void 0)return r;let{array:o,_indexes:i}=e,c=o.push(n);return i[n]=c-1}var Oa=0,ja=1,Fa=2,Ma=3,Ba=4,Lo=-1,Do=class{constructor({file:e,sourceRoot:n}={}){this._names=new hs,this._sources=new hs,this._sourcesContent=[],this._mappings=[],this.file=e,this.sourceRoot=n,this._ignoreList=new hs}};var ln=(e,n,r,o,i,c,p,h)=>$a(!0,e,n,r,o,i,c,p,h);function qa(e){let{_mappings:n,_sources:r,_sourcesContent:o,_names:i,_ignoreList:c}=e;return Va(n),{version:3,file:e.file||void 0,names:i.array,sourceRoot:e.sourceRoot||void 0,sources:r.array,sourcesContent:o,mappings:n,ignoreList:c.array}}function Oo(e){let n=qa(e);return Object.assign({},n,{mappings:fs(n.mappings)})}function $a(e,n,r,o,i,c,p,h,x){let{_mappings:k,_sources:y,_sourcesContent:w,_names:I}=n,A=Ua(k,r),O=Ha(A,o);if(!i)return e&&Wa(A,O)?void 0:Ro(A,O,[o]);let E=No(y,i),$=h?No(I,h):Lo;if(E===w.length&&(w[E]=x??null),!(e&&za(A,O,E,c,p,$)))return Ro(A,O,h?[o,E,c,p,$]:[o,E,c,p])}function Ua(e,n){for(let r=e.length;r<=n;r++)e[r]=[];return e[n]}function Ha(e,n){let r=e.length;for(let o=r-1;o>=0;r=o--){let i=e[o];if(n>=i[Oa])break}return r}function Ro(e,n,r){for(let o=e.length;o>n;o--)e[o]=e[o-1];e[n]=r}function Va(e){let{length:n}=e,r=n;for(let o=r-1;o>=0&&!(e[o].length>0);r=o,o--);r<n&&(e.length=r)}function Wa(e,n){return n===0?!0:e[n-1].length===1}function za(e,n,r,o,i,c){if(n===0)return!1;let p=e[n-1];return p.length===1?!1:r===p[ja]&&o===p[Fa]&&i===p[Ma]&&c===(p.length===5?p[Ba]:Lo)}function ms({code:e,mappings:n},r,o,i,c){let p=Ga(i,c),h=new Do({file:o.compiledFilename}),x=0,k=n[0];for(;k===void 0&&x<n.length-1;)x++,k=n[x];let y=0,w=0;k!==w&&ln(h,y,0,r,y,0);for(let E=0;E<e.length;E++){if(E===k){let $=k-w,Q=p[x];for(ln(h,y,$,r,y,Q);(k===E||k===void 0)&&x<n.length-1;)x++,k=n[x]}e.charCodeAt(E)===u.lineFeed&&(y++,w=E+1,k!==w&&ln(h,y,0,r,y,0))}let{sourceRoot:I,sourcesContent:A,...O}=Oo(h);return O}function Ga(e,n){let r=new Array(n.length),o=0,i=n[o].start,c=0;for(let p=0;p<e.length;p++)p===i&&(r[o]=i-c,o++,i=n[o].start),e.charCodeAt(p)===u.lineFeed&&(c=p+1);return r}var Xa={require:`
    import {createRequire as CREATE_REQUIRE_NAME} from "module";
    const require = CREATE_REQUIRE_NAME(import.meta.url);
  `,interopRequireWildcard:`
    function interopRequireWildcard(obj) {
      if (obj && obj.__esModule) {
        return obj;
      } else {
        var newObj = {};
        if (obj != null) {
          for (var key in obj) {
            if (Object.prototype.hasOwnProperty.call(obj, key)) {
              newObj[key] = obj[key];
            }
          }
        }
        newObj.default = obj;
        return newObj;
      }
    }
  `,interopRequireDefault:`
    function interopRequireDefault(obj) {
      return obj && obj.__esModule ? obj : { default: obj };
    }
  `,createNamedExportFrom:`
    function createNamedExportFrom(obj, localName, importedName) {
      Object.defineProperty(exports, localName, {enumerable: true, configurable: true, get: () => obj[importedName]});
    }
  `,createStarExport:`
    function createStarExport(obj) {
      Object.keys(obj)
        .filter((key) => key !== "default" && key !== "__esModule")
        .forEach((key) => {
          if (exports.hasOwnProperty(key)) {
            return;
          }
          Object.defineProperty(exports, key, {enumerable: true, configurable: true, get: () => obj[key]});
        });
    }
  `,nullishCoalesce:`
    function nullishCoalesce(lhs, rhsFn) {
      if (lhs != null) {
        return lhs;
      } else {
        return rhsFn();
      }
    }
  `,asyncNullishCoalesce:`
    async function asyncNullishCoalesce(lhs, rhsFn) {
      if (lhs != null) {
        return lhs;
      } else {
        return await rhsFn();
      }
    }
  `,optionalChain:`
    function optionalChain(ops) {
      let lastAccessLHS = undefined;
      let value = ops[0];
      let i = 1;
      while (i < ops.length) {
        const op = ops[i];
        const fn = ops[i + 1];
        i += 2;
        if ((op === 'optionalAccess' || op === 'optionalCall') && value == null) {
          return undefined;
        }
        if (op === 'access' || op === 'optionalAccess') {
          lastAccessLHS = value;
          value = fn(value);
        } else if (op === 'call' || op === 'optionalCall') {
          value = fn((...args) => value.call(lastAccessLHS, ...args));
          lastAccessLHS = undefined;
        }
      }
      return value;
    }
  `,asyncOptionalChain:`
    async function asyncOptionalChain(ops) {
      let lastAccessLHS = undefined;
      let value = ops[0];
      let i = 1;
      while (i < ops.length) {
        const op = ops[i];
        const fn = ops[i + 1];
        i += 2;
        if ((op === 'optionalAccess' || op === 'optionalCall') && value == null) {
          return undefined;
        }
        if (op === 'access' || op === 'optionalAccess') {
          lastAccessLHS = value;
          value = await fn(value);
        } else if (op === 'call' || op === 'optionalCall') {
          value = await fn((...args) => value.call(lastAccessLHS, ...args));
          lastAccessLHS = undefined;
        }
      }
      return value;
    }
  `,optionalChainDelete:`
    function optionalChainDelete(ops) {
      const result = OPTIONAL_CHAIN_NAME(ops);
      return result == null ? true : result;
    }
  `,asyncOptionalChainDelete:`
    async function asyncOptionalChainDelete(ops) {
      const result = await ASYNC_OPTIONAL_CHAIN_NAME(ops);
      return result == null ? true : result;
    }
  `},un=class e{__init(){this.helperNames={}}__init2(){this.createRequireName=null}constructor(n){this.nameManager=n,e.prototype.__init.call(this),e.prototype.__init2.call(this)}getHelperName(n){let r=this.helperNames[n];return r||(r=this.nameManager.claimFreeName(`_${n}`),this.helperNames[n]=r,r)}emitHelpers(){let n="";this.helperNames.optionalChainDelete&&this.getHelperName("optionalChain"),this.helperNames.asyncOptionalChainDelete&&this.getHelperName("asyncOptionalChain");for(let[r,o]of Object.entries(Xa)){let i=this.helperNames[r],c=o;r==="optionalChainDelete"?c=c.replace("OPTIONAL_CHAIN_NAME",this.helperNames.optionalChain):r==="asyncOptionalChainDelete"?c=c.replace("ASYNC_OPTIONAL_CHAIN_NAME",this.helperNames.asyncOptionalChain):r==="require"&&(this.createRequireName===null&&(this.createRequireName=this.nameManager.claimFreeName("_createRequire")),c=c.replace(/CREATE_REQUIRE_NAME/g,this.createRequireName)),i&&(n+=" ",n+=c.replace(r,i).replace(/\s+/g," ").trim())}return n}};function pn(e,n,r){Ja(e,r)&&Ka(e,n,r)}function Ja(e,n){for(let r of e.tokens)if(r.type===t.name&&!r.isType&&ko(r)&&n.has(e.identifierNameForToken(r)))return!0;return!1}function Ka(e,n,r){let o=[],i=n.length-1;for(let c=e.tokens.length-1;;c--){for(;o.length>0&&o[o.length-1].startTokenIndex===c+1;)o.pop();for(;i>=0&&n[i].endTokenIndex===c+1;)o.push(n[i]),i--;if(c<0)break;let p=e.tokens[c],h=e.identifierNameForToken(p);if(o.length>1&&!p.isType&&p.type===t.name&&r.has(h)){if(go(p))jo(o[o.length-1],e,h);else if(xo(p)){let x=o.length-1;for(;x>0&&!o[x].isFunctionScope;)x--;if(x<0)throw new Error("Did not find parent function scope.");jo(o[x],e,h)}}}if(o.length>0)throw new Error("Expected empty scope stack after processing file.")}function jo(e,n,r){for(let o=e.startTokenIndex;o<e.endTokenIndex;o++){let i=n.tokens[o];(i.type===t.name||i.type===t.jsxName)&&n.identifierNameForToken(i)===r&&(i.shadowsGlobal=!0)}}function ds(e,n){let r=[];for(let o of n)o.type===t.name&&r.push(e.slice(o.start,o.end));return r}var It=class e{__init(){this.usedNames=new Set}constructor(n,r){e.prototype.__init.call(this),this.usedNames=new Set(ds(n,r))}claimFreeName(n){let r=this.findFreeName(n);return this.usedNames.add(r),r}findFreeName(n){if(!this.usedNames.has(n))return n;let r=2;for(;this.usedNames.has(n+String(r));)r++;return n+String(r)}};var ei=Xr(ws());var F=Xr(ws()),xc=F.union(F.lit("jsx"),F.lit("typescript"),F.lit("flow"),F.lit("imports"),F.lit("react-hot-loader"),F.lit("jest")),yc=F.iface([],{compiledFilename:"string"}),_c=F.iface([],{transforms:F.array("Transform"),disableESTransforms:F.opt("boolean"),jsxRuntime:F.opt(F.union(F.lit("classic"),F.lit("automatic"),F.lit("preserve"))),production:F.opt("boolean"),jsxImportSource:F.opt("string"),jsxPragma:F.opt("string"),jsxFragmentPragma:F.opt("string"),keepUnusedImports:F.opt("boolean"),preserveDynamicImport:F.opt("boolean"),injectCreateRequireForImportRequire:F.opt("boolean"),enableLegacyTypeScriptModuleInterop:F.opt("boolean"),enableLegacyBabel5ModuleInterop:F.opt("boolean"),sourceMapOptions:F.opt("SourceMapOptions"),filePath:F.opt("string")}),bc={Transform:xc,SourceMapOptions:yc,Options:_c},Zo=bc;var{Options:Tc}=(0,ei.createCheckers)(Zo);function ti(e){Tc.strictCheck(e)}function vs(){d(),z(!1)}function Ss(e){d(),St(e)}function pe(e){v(),mn(e)}function it(){v(),s.tokens[s.tokens.length-1].identifierRole=b.ImportDeclaration}function mn(e){let n;s.scopeDepth===0?n=b.TopLevelDeclaration:e?n=b.BlockScopedDeclaration:n=b.FunctionScopedDeclaration,s.tokens[s.tokens.length-1].identifierRole=n}function St(e){switch(s.type){case t._this:{let n=N(0);d(),C(n);return}case t._yield:case t.name:{s.type=t.name,pe(e);return}case t.bracketL:{d(),Et(t.bracketR,e,!0);return}case t.braceL:Pt(!0,e);return;default:S()}}function Et(e,n,r=!1,o=!1,i=0){let c=!0,p=!1,h=s.tokens.length;for(;!f(e)&&!s.error;)if(c?c=!1:(m(t.comma),s.tokens[s.tokens.length-1].contextId=i,!p&&s.tokens[h].isType&&(s.tokens[s.tokens.length-1].isType=!0,p=!0)),!(r&&a(t.comma))){if(f(e))break;if(a(t.ellipsis)){Ss(n),ni(),f(t.comma),m(e);break}else Ic(o,n)}}function Ic(e,n){e&&At([l._public,l._protected,l._private,l._readonly,l._override]),vt(n),ni(),vt(n,!0)}function ni(){L?si():R&&ri()}function vt(e,n=!1){if(n||St(e),!f(t.eq))return;let r=s.tokens.length-1;z(),s.tokens[r].rhsEndIndex=s.tokens.length}function As(){return a(t.name)}function wc(){return a(t.name)||!!(s.type&t.IS_KEYWORD)||a(t.string)||a(t.num)||a(t.bigint)||a(t.decimal)}function ui(){let e=s.snapshot();return d(),(a(t.bracketL)||a(t.braceL)||a(t.star)||a(t.ellipsis)||a(t.hash)||wc())&&!X()?!0:(s.restoreFromSnapshot(e),!1)}function At(e){for(;pi(e)!==null;);}function pi(e){if(!a(t.name))return null;let n=s.contextualKeyword;if(e.indexOf(n)!==-1&&ui()){switch(n){case l._readonly:s.tokens[s.tokens.length-1].type=t._readonly;break;case l._abstract:s.tokens[s.tokens.length-1].type=t._abstract;break;case l._static:s.tokens[s.tokens.length-1].type=t._static;break;case l._public:s.tokens[s.tokens.length-1].type=t._public;break;case l._private:s.tokens[s.tokens.length-1].type=t._private;break;case l._protected:s.tokens[s.tokens.length-1].type=t._protected;break;case l._override:s.tokens[s.tokens.length-1].type=t._override;break;case l._declare:s.tokens[s.tokens.length-1].type=t._declare;break;default:break}return n}return null}function Nt(){for(v();f(t.dot);)v()}function vc(){Nt(),!X()&&a(t.lessThan)&&lt()}function Sc(){d(),ct()}function Ec(){d()}function Ac(){m(t._typeof),a(t._import)?fi():Nt(),!X()&&a(t.lessThan)&&lt()}function fi(){m(t._import),m(t.parenL),m(t.string),m(t.parenR),f(t.dot)&&Nt(),a(t.lessThan)&&lt()}function Pc(){f(t._const);let e=f(t._in),n=W(l._out);f(t._const),(e||n)&&!a(t.name)?s.tokens[s.tokens.length-1].type=t.name:v(),f(t._extends)&&V(),f(t.eq)&&V()}function Ce(){a(t.lessThan)&&kn()}function kn(){let e=N(0);for(a(t.lessThan)||a(t.typeParameterStart)?d():S();!f(t.greaterThan)&&!s.error;)Pc(),f(t.comma);C(e)}function Ns(e){let n=e===t.arrow;Ce(),m(t.parenL),s.scopeDepth++,Cc(!1),s.scopeDepth--,(n||a(e))&&Ct(e)}function Cc(e){Et(t.parenR,e)}function dn(){f(t.comma)||M()}function oi(){Ns(t.colon),dn()}function Nc(){let e=s.snapshot();d();let n=f(t.name)&&a(t.colon);return s.restoreFromSnapshot(e),n}function hi(){if(!(a(t.bracketL)&&Nc()))return!1;let e=N(0);return m(t.bracketL),v(),ct(),m(t.bracketR),Le(),dn(),C(e),!0}function ii(e){f(t.question),!e&&(a(t.parenL)||a(t.lessThan))?(Ns(t.colon),dn()):(Le(),dn())}function Rc(){if(a(t.parenL)||a(t.lessThan)){oi();return}if(a(t._new)){d(),a(t.parenL)||a(t.lessThan)?oi():ii(!1);return}let e=!!pi([l._readonly]);hi()||((_(l._get)||_(l._set))&&ui(),De(-1),ii(e))}function Lc(){mi()}function mi(){for(m(t.braceL);!f(t.braceR)&&!s.error;)Rc()}function Dc(){let e=s.snapshot(),n=Oc();return s.restoreFromSnapshot(e),n}function Oc(){return d(),f(t.plus)||f(t.minus)?_(l._readonly):(_(l._readonly)&&d(),!a(t.bracketL)||(d(),!As())?!1:(d(),a(t._in)))}function jc(){v(),m(t._in),V()}function Fc(){m(t.braceL),a(t.plus)||a(t.minus)?(d(),U(l._readonly)):W(l._readonly),m(t.bracketL),jc(),W(l._as)&&V(),m(t.bracketR),a(t.plus)||a(t.minus)?(d(),m(t.question)):f(t.question),Qc(),M(),m(t.braceR)}function Mc(){for(m(t.bracketL);!f(t.bracketR)&&!s.error;)Bc(),f(t.comma)}function Bc(){f(t.ellipsis)?V():(V(),f(t.question)),f(t.colon)&&V()}function qc(){m(t.parenL),V(),m(t.parenR)}function $c(){for(he(),he();!a(t.backQuote)&&!s.error;)m(t.dollarBraceL),V(),he(),he();d()}var Ae;(function(e){e[e.TSFunctionType=0]="TSFunctionType";let r=1;e[e.TSConstructorType=r]="TSConstructorType";let o=r+1;e[e.TSAbstractConstructorType=o]="TSAbstractConstructorType"})(Ae||(Ae={}));function Es(e){e===Ae.TSAbstractConstructorType&&U(l._abstract),(e===Ae.TSConstructorType||e===Ae.TSAbstractConstructorType)&&m(t._new);let n=s.inDisallowConditionalTypesContext;s.inDisallowConditionalTypesContext=!1,Ns(t.arrow),s.inDisallowConditionalTypesContext=n}function Uc(){switch(s.type){case t.name:vc();return;case t._void:case t._null:d();return;case t.string:case t.num:case t.bigint:case t.decimal:case t._true:case t._false:Pe();return;case t.minus:d(),Pe();return;case t._this:{Ec(),_(l._is)&&!X()&&Sc();return}case t._typeof:Ac();return;case t._import:fi();return;case t.braceL:Dc()?Fc():Lc();return;case t.bracketL:Mc();return;case t.parenL:qc();return;case t.backQuote:$c();return;default:if(s.type&t.IS_KEYWORD){d(),s.tokens[s.tokens.length-1].type=t.name;return}break}S()}function Hc(){for(Uc();!X()&&f(t.bracketL);)f(t.bracketR)||(V(),m(t.bracketR))}function Vc(){if(U(l._infer),v(),a(t._extends)){let e=s.snapshot();m(t._extends);let n=s.inDisallowConditionalTypesContext;s.inDisallowConditionalTypesContext=!0,V(),s.inDisallowConditionalTypesContext=n,(s.error||!s.inDisallowConditionalTypesContext&&a(t.question))&&s.restoreFromSnapshot(e)}}function Ps(){if(_(l._keyof)||_(l._unique)||_(l._readonly))d(),Ps();else if(_(l._infer))Vc();else{let e=s.inDisallowConditionalTypesContext;s.inDisallowConditionalTypesContext=!1,Hc(),s.inDisallowConditionalTypesContext=e}}function ai(){if(f(t.bitwiseAND),Ps(),a(t.bitwiseAND))for(;f(t.bitwiseAND);)Ps()}function Wc(){if(f(t.bitwiseOR),ai(),a(t.bitwiseOR))for(;f(t.bitwiseOR);)ai()}function zc(){return a(t.lessThan)?!0:a(t.parenL)&&Xc()}function Gc(){if(a(t.name)||a(t._this))return d(),!0;if(a(t.braceL)||a(t.bracketL)){let e=1;for(d();e>0&&!s.error;)a(t.braceL)||a(t.bracketL)?e++:(a(t.braceR)||a(t.bracketR))&&e--,d();return!0}return!1}function Xc(){let e=s.snapshot(),n=Jc();return s.restoreFromSnapshot(e),n}function Jc(){return d(),!!(a(t.parenR)||a(t.ellipsis)||Gc()&&(a(t.colon)||a(t.comma)||a(t.question)||a(t.eq)||a(t.parenR)&&(d(),a(t.arrow))))}function Ct(e){let n=N(0);m(e),Yc()||V(),C(n)}function Kc(){a(t.colon)&&Ct(t.colon)}function Le(){a(t.colon)&&ct()}function Qc(){f(t.colon)&&V()}function Yc(){let e=s.snapshot();return _(l._asserts)?(d(),W(l._is)?(V(),!0):As()||a(t._this)?(d(),W(l._is)&&V(),!0):(s.restoreFromSnapshot(e),!1)):As()||a(t._this)?(d(),_(l._is)&&!X()?(d(),V(),!0):(s.restoreFromSnapshot(e),!1)):!1}function ct(){let e=N(0);m(t.colon),V(),C(e)}function V(){if(ci(),s.inDisallowConditionalTypesContext||X()||!f(t._extends))return;let e=s.inDisallowConditionalTypesContext;s.inDisallowConditionalTypesContext=!0,ci(),s.inDisallowConditionalTypesContext=e,m(t.question),V(),m(t.colon),V()}function Zc(){return _(l._abstract)&&B()===t._new}function ci(){if(zc()){Es(Ae.TSFunctionType);return}if(a(t._new)){Es(Ae.TSConstructorType);return}else if(Zc()){Es(Ae.TSAbstractConstructorType);return}Wc()}function di(){let e=N(1);V(),m(t.greaterThan),C(e),ut()}function ki(){if(f(t.jsxTagStart)){s.tokens[s.tokens.length-1].type=t.typeParameterStart;let e=N(1);for(;!a(t.greaterThan)&&!s.error;)V(),f(t.comma);oe(),C(e)}}function gi(){for(;!a(t.braceL)&&!s.error;)el(),f(t.comma)}function el(){Nt(),a(t.lessThan)&&lt()}function tl(){pe(!1),Ce(),f(t._extends)&&gi(),mi()}function nl(){pe(!1),Ce(),m(t.eq),V(),M()}function rl(){if(a(t.string)?Pe():v(),f(t.eq)){let e=s.tokens.length-1;z(),s.tokens[e].rhsEndIndex=s.tokens.length}}function Rs(){for(pe(!1),m(t.braceL);!f(t.braceR)&&!s.error;)rl(),f(t.comma)}function Ls(){m(t.braceL),pt(t.braceR)}function Cs(){pe(!1),f(t.dot)?Cs():Ls()}function xi(){_(l._global)?v():a(t.string)?le():S(),a(t.braceL)?Ls():M()}function gn(){it(),m(t.eq),ol(),M()}function sl(){return _(l._require)&&B()===t.parenL}function ol(){sl()?il():Nt()}function il(){U(l._require),m(t.parenL),a(t.string)||S(),Pe(),m(t.parenR)}function al(){if(ae())return!1;switch(s.type){case t._function:{let e=N(1);d();let n=s.start;return _e(n,!0),C(e),!0}case t._class:{let e=N(1);return Te(!0,!1),C(e),!0}case t._const:if(a(t._const)&&st(l._enum)){let e=N(1);return m(t._const),U(l._enum),s.tokens[s.tokens.length-1].type=t._enum,Rs(),C(e),!0}case t._var:case t._let:{let e=N(1);return Lt(s.type!==t._var),C(e),!0}case t.name:{let e=N(1),n=s.contextualKeyword,r=!1;return n===l._global?(xi(),r=!0):r=xn(n,!0),C(e),r}default:return!1}}function li(){return xn(s.contextualKeyword,!0)}function cl(e){switch(e){case l._declare:{let n=s.tokens.length-1;if(al())return s.tokens[n].type=t._declare,!0;break}case l._global:if(a(t.braceL))return Ls(),!0;break;default:return xn(e,!1)}return!1}function xn(e,n){switch(e){case l._abstract:if(at(n)&&a(t._class))return s.tokens[s.tokens.length-1].type=t._abstract,Te(!0,!1),!0;break;case l._enum:if(at(n)&&a(t.name))return s.tokens[s.tokens.length-1].type=t._enum,Rs(),!0;break;case l._interface:if(at(n)&&a(t.name)){let r=N(n?2:1);return tl(),C(r),!0}break;case l._module:if(at(n)){if(a(t.string)){let r=N(n?2:1);return xi(),C(r),!0}else if(a(t.name)){let r=N(n?2:1);return Cs(),C(r),!0}}break;case l._namespace:if(at(n)&&a(t.name)){let r=N(n?2:1);return Cs(),C(r),!0}break;case l._type:if(at(n)&&a(t.name)){let r=N(n?2:1);return nl(),C(r),!0}break;default:break}return!1}function at(e){return e?(d(),!0):!ae()}function ll(){let e=s.snapshot();return kn(),be(),Kc(),m(t.arrow),s.error?(s.restoreFromSnapshot(e),!1):(Oe(!0),!0)}function Ds(){s.type===t.bitShiftL&&(s.pos-=1,P(t.lessThan)),lt()}function lt(){let e=N(0);for(m(t.lessThan);!a(t.greaterThan)&&!s.error;)V(),f(t.comma);e?(m(t.greaterThan),C(e)):(C(e),on(),m(t.greaterThan),s.tokens[s.tokens.length-1].isType=!0)}function Os(){if(a(t.name))switch(s.contextualKeyword){case l._abstract:case l._declare:case l._enum:case l._interface:case l._module:case l._namespace:case l._type:return!0;default:break}return!1}function yi(e,n){if(a(t.colon)&&Ct(t.colon),!a(t.braceL)&&ae()){let r=s.tokens.length-1;for(;r>=0&&(s.tokens[r].start>=e||s.tokens[r].type===t._default||s.tokens[r].type===t._export);)s.tokens[r].isType=!0,r--;return}Oe(!1,n)}function _i(e,n,r){if(!X()&&f(t.bang)){s.tokens[s.tokens.length-1].type=t.nonNullAssertion;return}if(a(t.lessThan)||a(t.bitShiftL)){let o=s.snapshot();if(!n&&js()&&ll())return;if(Ds(),!n&&f(t.parenL)?(s.tokens[s.tokens.length-1].subscriptStartIndex=e,fe()):a(t.backQuote)?yn():(s.type===t.greaterThan||s.type!==t.parenL&&s.type&t.IS_EXPRESSION_START&&!X())&&S(),s.error)s.restoreFromSnapshot(o);else return}else!n&&a(t.questionDot)&&B()===t.lessThan&&(d(),s.tokens[e].isOptionalChainStart=!0,s.tokens[s.tokens.length-1].subscriptStartIndex=e,lt(),m(t.parenL),fe());Rt(e,n,r)}function bi(){if(f(t._import))return _(l._type)&&B()!==t.eq&&U(l._type),gn(),!0;if(f(t.eq))return G(),M(),!0;if(W(l._as))return U(l._namespace),v(),M(),!0;if(_(l._type)){let e=B();(e===t.braceL||e===t.star)&&d()}return!1}function Ti(){if(v(),a(t.comma)||a(t.braceR)){s.tokens[s.tokens.length-1].identifierRole=b.ImportDeclaration;return}if(v(),a(t.comma)||a(t.braceR)){s.tokens[s.tokens.length-1].identifierRole=b.ImportDeclaration,s.tokens[s.tokens.length-2].isType=!0,s.tokens[s.tokens.length-1].isType=!0;return}if(v(),a(t.comma)||a(t.braceR)){s.tokens[s.tokens.length-3].identifierRole=b.ImportAccess,s.tokens[s.tokens.length-1].identifierRole=b.ImportDeclaration;return}v(),s.tokens[s.tokens.length-3].identifierRole=b.ImportAccess,s.tokens[s.tokens.length-1].identifierRole=b.ImportDeclaration,s.tokens[s.tokens.length-4].isType=!0,s.tokens[s.tokens.length-3].isType=!0,s.tokens[s.tokens.length-2].isType=!0,s.tokens[s.tokens.length-1].isType=!0}function Ii(){if(v(),a(t.comma)||a(t.braceR)){s.tokens[s.tokens.length-1].identifierRole=b.ExportAccess;return}if(v(),a(t.comma)||a(t.braceR)){s.tokens[s.tokens.length-1].identifierRole=b.ExportAccess,s.tokens[s.tokens.length-2].isType=!0,s.tokens[s.tokens.length-1].isType=!0;return}if(v(),a(t.comma)||a(t.braceR)){s.tokens[s.tokens.length-3].identifierRole=b.ExportAccess;return}v(),s.tokens[s.tokens.length-3].identifierRole=b.ExportAccess,s.tokens[s.tokens.length-4].isType=!0,s.tokens[s.tokens.length-3].isType=!0,s.tokens[s.tokens.length-2].isType=!0,s.tokens[s.tokens.length-1].isType=!0}function wi(){if(_(l._abstract)&&B()===t._class)return s.type=t._abstract,d(),Te(!0,!0),!0;if(_(l._interface)){let e=N(2);return xn(l._interface,!0),C(e),!0}return!1}function vi(){if(s.type===t._const){let e=xe();if(e.type===t.name&&e.contextualKeyword===l._enum)return m(t._const),U(l._enum),s.tokens[s.tokens.length-1].type=t._enum,Rs(),!0}return!1}function Si(e){let n=s.tokens.length;At([l._abstract,l._readonly,l._declare,l._static,l._override]);let r=s.tokens.length;if(hi()){let i=e?n-1:n;for(let c=i;c<r;c++)s.tokens[c].isType=!0;return!0}return!1}function Ei(e){cl(e)||M()}function Ai(){let e=W(l._declare);e&&(s.tokens[s.tokens.length-1].type=t._declare);let n=!1;if(a(t.name))if(e){let r=N(2);n=li(),C(r)}else n=li();if(!n)if(e){let r=N(2);Z(!0),C(r)}else Z(!0)}function Pi(e){if(e&&(a(t.lessThan)||a(t.bitShiftL))&&Ds(),W(l._implements)){s.tokens[s.tokens.length-1].type=t._implements;let n=N(1);gi(),C(n)}}function Ci(){Ce()}function Ni(){Ce()}function Ri(){let e=N(0);X()||f(t.bang),Le(),C(e)}function Li(){a(t.colon)&&ct()}function Di(e,n){return rt?ul(e,n):pl(e,n)}function ul(e,n){if(!a(t.lessThan))return ce(e,n);let r=s.snapshot(),o=ce(e,n);if(s.error)s.restoreFromSnapshot(r);else return o;return s.type=t.typeParameterStart,kn(),o=ce(e,n),o||S(),o}function pl(e,n){if(!a(t.lessThan))return ce(e,n);let r=s.snapshot();kn();let o=ce(e,n);if(o||S(),s.error)s.restoreFromSnapshot(r);else return o;return ce(e,n)}function Oi(){if(a(t.colon)){let e=s.snapshot();Ct(t.colon),J()&&S(),a(t.arrow)||S(),s.error&&s.restoreFromSnapshot(e)}return f(t.arrow)}function ri(){let e=N(0);f(t.question),Le(),C(e)}function ji(){(a(t.lessThan)||a(t.bitShiftL))&&Ds(),Fs()}function fl(){let e=!1,n=!1;for(;;){if(s.pos>=g.length){S("Unterminated JSX contents");return}let r=g.charCodeAt(s.pos);if(r===u.lessThan||r===u.leftCurlyBrace){if(s.pos===s.start){if(r===u.lessThan){s.pos++,P(t.jsxTagStart);return}us(r);return}e&&!n?P(t.jsxEmptyText):P(t.jsxText);return}r===u.lineFeed?e=!0:r!==u.space&&r!==u.carriageReturn&&r!==u.tab&&(n=!0),s.pos++}}function hl(e){for(s.pos++;;){if(s.pos>=g.length){S("Unterminated string constant");return}if(g.charCodeAt(s.pos)===e){s.pos++;break}s.pos++}P(t.string)}function ml(){let e;do{if(s.pos>g.length){S("Unexpectedly reached the end of input.");return}e=g.charCodeAt(++s.pos)}while(Y[e]||e===u.dash);P(t.jsxName)}function Ms(){oe()}function Fi(e){if(Ms(),!f(t.colon)){s.tokens[s.tokens.length-1].identifierRole=e;return}Ms()}function Mi(){let e=s.tokens.length;Fi(b.Access);let n=!1;for(;a(t.dot);)n=!0,oe(),Ms();if(!n){let r=s.tokens[e],o=g.charCodeAt(r.start);o>=u.lowercaseA&&o<=u.lowercaseZ&&(r.identifierRole=null)}}function dl(){switch(s.type){case t.braceL:d(),G(),oe();return;case t.jsxTagStart:Bs(),oe();return;case t.string:oe();return;default:S("JSX value should be either an expression or a quoted JSX text")}}function kl(){m(t.ellipsis),G()}function gl(e){if(a(t.jsxTagEnd))return!1;Mi(),R&&ki();let n=!1;for(;!a(t.slash)&&!a(t.jsxTagEnd)&&!s.error;){if(f(t.braceL)){n=!0,m(t.ellipsis),z(),oe();continue}n&&s.end-s.start===3&&g.charCodeAt(s.start)===u.lowercaseK&&g.charCodeAt(s.start+1)===u.lowercaseE&&g.charCodeAt(s.start+2)===u.lowercaseY&&(s.tokens[e].jsxRole=re.KeyAfterPropSpread),Fi(b.ObjectKey),a(t.eq)&&(oe(),dl())}let r=a(t.slash);return r&&oe(),r}function xl(){a(t.jsxTagEnd)||Mi()}function Bi(){let e=s.tokens.length-1;s.tokens[e].jsxRole=re.NoChildren;let n=0;if(!gl(e))for(ft();;)switch(s.type){case t.jsxTagStart:if(oe(),a(t.slash)){oe(),xl(),s.tokens[e].jsxRole!==re.KeyAfterPropSpread&&(n===1?s.tokens[e].jsxRole=re.OneChild:n>1&&(s.tokens[e].jsxRole=re.StaticChildren));return}n++,Bi(),ft();break;case t.jsxText:n++,ft();break;case t.jsxEmptyText:ft();break;case t.braceL:d(),a(t.ellipsis)?(kl(),ft(),n+=2):(a(t.braceR)||(n++,G()),ft());break;default:S();return}}function Bs(){oe(),Bi()}function oe(){s.tokens.push(new Re),ls(),s.start=s.pos;let e=g.charCodeAt(s.pos);if(ye[e])ml();else if(e===u.quotationMark||e===u.apostrophe)hl(e);else switch(++s.pos,e){case u.greaterThan:P(t.jsxTagEnd);break;case u.lessThan:P(t.jsxTagStart);break;case u.slash:P(t.slash);break;case u.equalsTo:P(t.eq);break;case u.leftCurlyBrace:P(t.braceL);break;case u.dot:P(t.dot);break;case u.colon:P(t.colon);break;default:S()}}function ft(){s.tokens.push(new Re),s.start=s.pos,fl()}function qi(e){if(a(t.question)){let n=B();if(n===t.colon||n===t.comma||n===t.parenR)return}qs(e)}function $i(){sn(t.question),a(t.colon)&&(R?ct():L&&Ie())}var $s=class{constructor(n){this.stop=n}};function G(e=!1){if(z(e),a(t.comma))for(;f(t.comma);)z(e)}function z(e=!1,n=!1){return R?Di(e,n):L?Ki(e,n):ce(e,n)}function ce(e,n){if(a(t._yield))return Dl(),!1;(a(t.parenL)||a(t.name)||a(t._yield))&&(s.potentialArrowAt=s.start);let r=yl(e);return n&&zs(),s.type&t.IS_ASSIGN?(d(),z(e),!1):r}function yl(e){return bl(e)?!0:(_l(e),!1)}function _l(e){R||L?qi(e):qs(e)}function qs(e){f(t.question)&&(z(),m(t.colon),z(e))}function bl(e){let n=s.tokens.length;return ut()?!0:(_n(n,-1,e),!1)}function _n(e,n,r){if(R&&(t._in&t.PRECEDENCE_MASK)>n&&!X()&&(W(l._as)||W(l._satisfies))){let i=N(1);V(),C(i),on(),_n(e,n,r);return}let o=s.type&t.PRECEDENCE_MASK;if(o>0&&(!r||!a(t._in))&&o>n){let i=s.type;d(),i===t.nullishCoalescing&&(s.tokens[s.tokens.length-1].nullishStartIndex=e);let c=s.tokens.length;ut(),_n(c,i&t.IS_RIGHT_ASSOCIATIVE?o-1:o,r),i===t.nullishCoalescing&&(s.tokens[e].numNullishCoalesceStarts++,s.tokens[s.tokens.length-1].numNullishCoalesceEnds++),_n(e,n,r)}}function ut(){if(R&&!rt&&f(t.lessThan))return di(),!1;if(_(l._module)&&is()===u.leftCurlyBrace&&!tn())return Ol(),!1;if(s.type&t.IS_PREFIX)return d(),ut(),!1;if(Us())return!0;for(;s.type&t.IS_POSTFIX&&!J();)s.type===t.preIncDec&&(s.type=t.postIncDec),d();return!1}function Us(){let e=s.tokens.length;return le()?!0:(Hs(e),s.tokens.length>e&&s.tokens[e].isOptionalChainStart&&(s.tokens[s.tokens.length-1].isOptionalChainEnd=!0),!1)}function Hs(e,n=!1){L?Yi(e,n):Vs(e,n)}function Vs(e,n=!1){let r=new $s(!1);do Tl(e,n,r);while(!r.stop&&!s.error)}function Tl(e,n,r){R?_i(e,n,r):L?zi(e,n,r):Rt(e,n,r)}function Rt(e,n,r){if(!n&&f(t.doubleColon))Ws(),r.stop=!0,Hs(e,n);else if(a(t.questionDot)){if(s.tokens[e].isOptionalChainStart=!0,n&&B()===t.parenL){r.stop=!0;return}d(),s.tokens[s.tokens.length-1].subscriptStartIndex=e,f(t.bracketL)?(G(),m(t.bracketR)):f(t.parenL)?fe():bn()}else if(f(t.dot))s.tokens[s.tokens.length-1].subscriptStartIndex=e,bn();else if(f(t.bracketL))s.tokens[s.tokens.length-1].subscriptStartIndex=e,G(),m(t.bracketR);else if(!n&&a(t.parenL))if(js()){let o=s.snapshot(),i=s.tokens.length;d(),s.tokens[s.tokens.length-1].subscriptStartIndex=e;let c=Ne();s.tokens[s.tokens.length-1].contextId=c,fe(),s.tokens[s.tokens.length-1].contextId=c,Il()&&(s.restoreFromSnapshot(o),r.stop=!0,s.scopeDepth++,be(),wl(i))}else{d(),s.tokens[s.tokens.length-1].subscriptStartIndex=e;let o=Ne();s.tokens[s.tokens.length-1].contextId=o,fe(),s.tokens[s.tokens.length-1].contextId=o}else a(t.backQuote)?yn():r.stop=!0}function js(){return s.tokens[s.tokens.length-1].contextualKeyword===l._async&&!J()}function fe(){let e=!0;for(;!f(t.parenR)&&!s.error;){if(e)e=!1;else if(m(t.comma),f(t.parenR))break;Vi(!1)}}function Il(){return a(t.colon)||a(t.arrow)}function wl(e){R?Li():L&&Ji(),m(t.arrow),ht(e)}function Ws(){let e=s.tokens.length;le(),Hs(e,!0)}function le(){if(f(t.modulo))return v(),!1;if(a(t.jsxText)||a(t.jsxEmptyText))return Pe(),!1;if(a(t.lessThan)&&rt)return s.type=t.jsxTagStart,Bs(),d(),!1;let e=s.potentialArrowAt===s.start;switch(s.type){case t.slash:case t.assign:_o();case t._super:case t._this:case t.regexp:case t.num:case t.bigint:case t.decimal:case t.string:case t._null:case t._true:case t._false:return d(),!1;case t._import:return d(),a(t.dot)&&(s.tokens[s.tokens.length-1].type=t.name,d(),v()),!1;case t.name:{let n=s.tokens.length,r=s.start,o=s.contextualKeyword;return v(),o===l._await?(Ll(),!1):o===l._async&&a(t._function)&&!J()?(d(),_e(r,!1),!1):e&&o===l._async&&!J()&&a(t.name)?(s.scopeDepth++,pe(!1),m(t.arrow),ht(n),!0):a(t._do)&&!J()?(d(),we(),!1):e&&!J()&&a(t.arrow)?(s.scopeDepth++,mn(!1),m(t.arrow),ht(n),!0):(s.tokens[s.tokens.length-1].identifierRole=b.Access,!1)}case t._do:return d(),we(),!1;case t.parenL:return Ui(e);case t.bracketL:return d(),Hi(t.bracketR,!0),!1;case t.braceL:return Pt(!1,!1),!1;case t._function:return vl(),!1;case t.at:vn();case t._class:return Te(!1),!1;case t._new:return El(),!1;case t.backQuote:return yn(),!1;case t.doubleColon:return d(),Ws(),!1;case t.hash:{let n=is();return ye[n]||n===u.backslash?bn():d(),!1}default:return S(),!1}}function bn(){f(t.hash),v()}function vl(){let e=s.start;v(),f(t.dot)&&v(),_e(e,!1)}function Pe(){d()}function Dt(){m(t.parenL),G(),m(t.parenR)}function Ui(e){let n=s.snapshot(),r=s.tokens.length;m(t.parenL);let o=!0;for(;!a(t.parenR)&&!s.error;){if(o)o=!1;else if(m(t.comma),a(t.parenR))break;if(a(t.ellipsis)){Ss(!1),zs();break}else z(!1,!0)}return m(t.parenR),e&&Sl()&&Tn()?(s.restoreFromSnapshot(n),s.scopeDepth++,be(),Tn(),ht(r),s.error?(s.restoreFromSnapshot(n),Ui(!1),!1):!0):!1}function Sl(){return a(t.colon)||!J()}function Tn(){return R?Oi():L?Qi():f(t.arrow)}function zs(){(R||L)&&$i()}function El(){if(m(t._new),f(t.dot)){v();return}Al(),L&&Gi(),f(t.parenL)&&Hi(t.parenR)}function Al(){Ws(),f(t.questionDot)}function yn(){for(he(),he();!a(t.backQuote)&&!s.error;)m(t.dollarBraceL),G(),he(),he();d()}function Pt(e,n){let r=Ne(),o=!0;for(d(),s.tokens[s.tokens.length-1].contextId=r;!f(t.braceR)&&!s.error;){if(o)o=!1;else if(m(t.comma),f(t.braceR))break;let i=!1;if(a(t.ellipsis)){let c=s.tokens.length;if(vs(),e&&(s.tokens.length===c+2&&mn(n),f(t.braceR)))break;continue}e||(i=f(t.star)),!e&&_(l._async)?(i&&S(),v(),a(t.colon)||a(t.parenL)||a(t.braceR)||a(t.eq)||a(t.comma)||(a(t.star)&&(d(),i=!0),De(r))):De(r),Rl(e,n,r)}s.tokens[s.tokens.length-1].contextId=r}function Pl(e){return!e&&(a(t.string)||a(t.num)||a(t.bracketL)||a(t.name)||!!(s.type&t.IS_KEYWORD))}function Cl(e,n){let r=s.start;return a(t.parenL)?(e&&S(),In(r,!1),!0):Pl(e)?(De(n),In(r,!1),!0):!1}function Nl(e,n){if(f(t.colon)){e?vt(n):z(!1);return}let r;e?s.scopeDepth===0?r=b.ObjectShorthandTopLevelDeclaration:n?r=b.ObjectShorthandBlockScopedDeclaration:r=b.ObjectShorthandFunctionScopedDeclaration:r=b.ObjectShorthand,s.tokens[s.tokens.length-1].identifierRole=r,vt(n,!0)}function Rl(e,n,r){R?Ci():L&&Xi(),Cl(e,r)||Nl(e,n)}function De(e){L&&wn(),f(t.bracketL)?(s.tokens[s.tokens.length-1].contextId=e,z(),m(t.bracketR),s.tokens[s.tokens.length-1].contextId=e):(a(t.num)||a(t.string)||a(t.bigint)||a(t.decimal)?le():bn(),s.tokens[s.tokens.length-1].identifierRole=b.ObjectKey,s.tokens[s.tokens.length-1].contextId=e)}function In(e,n){let r=Ne();s.scopeDepth++;let o=s.tokens.length;be(n,r),Gs(e,r);let c=s.tokens.length;s.scopes.push(new ee(o,c,!0)),s.scopeDepth--}function ht(e){Oe(!0);let n=s.tokens.length;s.scopes.push(new ee(e,n,!0)),s.scopeDepth--}function Gs(e,n=0){R?yi(e,n):L?Wi(n):Oe(!1,n)}function Oe(e,n=0){e&&!a(t.braceL)?z():we(!0,n)}function Hi(e,n=!1){let r=!0;for(;!f(e)&&!s.error;){if(r)r=!1;else if(m(t.comma),f(e))break;Vi(n)}}function Vi(e){e&&a(t.comma)||(a(t.ellipsis)?(vs(),zs()):a(t.question)?d():z(!1,!0))}function v(){d(),s.tokens[s.tokens.length-1].type=t.name}function Ll(){ut()}function Dl(){d(),!a(t.semi)&&!J()&&(f(t.star),z())}function Ol(){U(l._module),m(t.braceL),pt(t.braceR)}function jl(e){return(e.type===t.name||!!(e.type&t.IS_KEYWORD))&&e.contextualKeyword!==l._from}function ke(e){let n=N(0);m(e||t.colon),ne(),C(n)}function Zi(){m(t.modulo),U(l._checks),f(t.parenL)&&(G(),m(t.parenR))}function Ks(){let e=N(0);m(t.colon),a(t.modulo)?Zi():(ne(),a(t.modulo)&&Zi()),C(e)}function Fl(){d(),Qs(!0)}function Ml(){d(),v(),a(t.lessThan)&&ue(),m(t.parenL),Js(),m(t.parenR),Ks(),M()}function Xs(){a(t._class)?Fl():a(t._function)?Ml():a(t._var)?Bl():W(l._module)?f(t.dot)?Ul():ql():_(l._type)?Hl():_(l._opaque)?Vl():_(l._interface)?Wl():a(t._export)?$l():S()}function Bl(){d(),o1(),M()}function ql(){for(a(t.string)?le():v(),m(t.braceL);!a(t.braceR)&&!s.error;)a(t._import)?(d(),so()):S();m(t.braceR)}function $l(){m(t._export),f(t._default)?a(t._function)||a(t._class)?Xs():(ne(),M()):a(t._var)||a(t._function)||a(t._class)||_(l._opaque)?Xs():a(t.star)||a(t.braceL)||_(l._interface)||_(l._type)||_(l._opaque)?ro():S()}function Ul(){U(l._exports),Ie(),M()}function Hl(){d(),Zs()}function Vl(){d(),eo(!0)}function Wl(){d(),Qs()}function Qs(e=!1){if(Cn(),a(t.lessThan)&&ue(),f(t._extends))do Sn();while(!e&&f(t.comma));if(_(l._mixins)){d();do Sn();while(f(t.comma))}if(_(l._implements)){d();do Sn();while(f(t.comma))}En(e,!1,e)}function Sn(){n1(!1),a(t.lessThan)&&je()}function Ys(){Qs()}function Cn(){v()}function Zs(){Cn(),a(t.lessThan)&&ue(),ke(t.eq),M()}function eo(e){U(l._type),Cn(),a(t.lessThan)&&ue(),a(t.colon)&&ke(t.colon),e||ke(t.eq),M()}function zl(){wn(),o1(),f(t.eq)&&ne()}function ue(){let e=N(0);a(t.lessThan)||a(t.typeParameterStart)?d():S();do zl(),a(t.greaterThan)||m(t.comma);while(!a(t.greaterThan)&&!s.error);m(t.greaterThan),C(e)}function je(){let e=N(0);for(m(t.lessThan);!a(t.greaterThan)&&!s.error;)ne(),a(t.greaterThan)||m(t.comma);m(t.greaterThan),C(e)}function Gl(){if(U(l._interface),f(t._extends))do Sn();while(f(t.comma));En(!1,!1,!1)}function to(){a(t.num)||a(t.string)?le():v()}function Xl(){B()===t.colon?(to(),ke()):ne(),m(t.bracketR),ke()}function Jl(){to(),m(t.bracketR),m(t.bracketR),a(t.lessThan)||a(t.parenL)?no():(f(t.question),ke())}function no(){for(a(t.lessThan)&&ue(),m(t.parenL);!a(t.parenR)&&!a(t.ellipsis)&&!s.error;)An(),a(t.parenR)||m(t.comma);f(t.ellipsis)&&An(),m(t.parenR),ke()}function Kl(){no()}function En(e,n,r){let o;for(n&&a(t.braceBarL)?(m(t.braceBarL),o=t.braceBarR):(m(t.braceL),o=t.braceR);!a(o)&&!s.error;){if(r&&_(l._proto)){let i=B();i!==t.colon&&i!==t.question&&(d(),e=!1)}if(e&&_(l._static)){let i=B();i!==t.colon&&i!==t.question&&d()}if(wn(),f(t.bracketL))f(t.bracketL)?Jl():Xl();else if(a(t.parenL)||a(t.lessThan))Kl();else{if(_(l._get)||_(l._set)){let i=B();(i===t.name||i===t.string||i===t.num)&&d()}Ql()}Yl()}m(o)}function Ql(){if(a(t.ellipsis)){if(m(t.ellipsis),f(t.comma)||f(t.semi),a(t.braceR))return;ne()}else to(),a(t.lessThan)||a(t.parenL)?no():(f(t.question),ke())}function Yl(){!f(t.semi)&&!f(t.comma)&&!a(t.braceR)&&!a(t.braceBarR)&&S()}function n1(e){for(e||v();f(t.dot);)v()}function Zl(){n1(!0),a(t.lessThan)&&je()}function eu(){m(t._typeof),r1()}function tu(){for(m(t.bracketL);s.pos<g.length&&!a(t.bracketR)&&(ne(),!a(t.bracketR));)m(t.comma);m(t.bracketR)}function An(){let e=B();e===t.colon||e===t.question?(v(),f(t.question),ke()):ne()}function Js(){for(;!a(t.parenR)&&!a(t.ellipsis)&&!s.error;)An(),a(t.parenR)||m(t.comma);f(t.ellipsis)&&An()}function r1(){let e=!1,n=s.noAnonFunctionType;switch(s.type){case t.name:{if(_(l._interface)){Gl();return}v(),Zl();return}case t.braceL:En(!1,!1,!1);return;case t.braceBarL:En(!1,!0,!1);return;case t.bracketL:tu();return;case t.lessThan:ue(),m(t.parenL),Js(),m(t.parenR),m(t.arrow),ne();return;case t.parenL:if(d(),!a(t.parenR)&&!a(t.ellipsis))if(a(t.name)){let r=B();e=r!==t.question&&r!==t.colon}else e=!0;if(e)if(s.noAnonFunctionType=!1,ne(),s.noAnonFunctionType=n,s.noAnonFunctionType||!(a(t.comma)||a(t.parenR)&&B()===t.arrow)){m(t.parenR);return}else f(t.comma);Js(),m(t.parenR),m(t.arrow),ne();return;case t.minus:d(),Pe();return;case t.string:case t.num:case t._true:case t._false:case t._null:case t._this:case t._void:case t.star:d();return;default:if(s.type===t._typeof){eu();return}else if(s.type&t.IS_KEYWORD){d(),s.tokens[s.tokens.length-1].type=t.name;return}}S()}function nu(){for(r1();!J()&&(a(t.bracketL)||a(t.questionDot));)f(t.questionDot),m(t.bracketL),f(t.bracketR)||(ne(),m(t.bracketR))}function s1(){f(t.question)?s1():nu()}function e1(){s1(),!s.noAnonFunctionType&&f(t.arrow)&&ne()}function t1(){for(f(t.bitwiseAND),e1();f(t.bitwiseAND);)e1()}function ru(){for(f(t.bitwiseOR),t1();f(t.bitwiseOR);)t1()}function ne(){ru()}function Ie(){ke()}function o1(){v(),a(t.colon)&&Ie()}function wn(){(a(t.plus)||a(t.minus))&&(d(),s.tokens[s.tokens.length-1].isType=!0)}function Wi(e){a(t.colon)&&Ks(),Oe(!1,e)}function zi(e,n,r){if(a(t.questionDot)&&B()===t.lessThan){if(n){r.stop=!0;return}d(),je(),m(t.parenL),fe();return}else if(!n&&a(t.lessThan)){let o=s.snapshot();if(je(),m(t.parenL),fe(),s.error)s.restoreFromSnapshot(o);else return}Rt(e,n,r)}function Gi(){if(a(t.lessThan)){let e=s.snapshot();je(),s.error&&s.restoreFromSnapshot(e)}}function i1(){if(a(t.name)&&s.contextualKeyword===l._interface){let e=N(0);return d(),Ys(),C(e),!0}else if(_(l._enum))return y1(),!0;return!1}function a1(){return _(l._enum)?(y1(),!0):!1}function c1(e){if(e===l._declare){if(a(t._class)||a(t.name)||a(t._function)||a(t._var)||a(t._export)){let n=N(1);Xs(),C(n)}}else if(a(t.name)){if(e===l._interface){let n=N(1);Ys(),C(n)}else if(e===l._type){let n=N(1);Zs(),C(n)}else if(e===l._opaque){let n=N(1);eo(!1),C(n)}}M()}function l1(){return _(l._type)||_(l._interface)||_(l._opaque)||_(l._enum)}function u1(){return a(t.name)&&(s.contextualKeyword===l._type||s.contextualKeyword===l._interface||s.contextualKeyword===l._opaque||s.contextualKeyword===l._enum)}function p1(){if(_(l._type)){let e=N(1);d(),a(t.braceL)?(Nn(),mt()):Zs(),C(e)}else if(_(l._opaque)){let e=N(1);d(),eo(!1),C(e)}else if(_(l._interface)){let e=N(1);d(),Ys(),C(e)}else Z(!0)}function f1(){return a(t.star)||_(l._type)&&B()===t.star}function h1(){if(W(l._type)){let e=N(2);Pn(),C(e)}else Pn()}function m1(e){if(e&&a(t.lessThan)&&je(),_(l._implements)){let n=N(0);d(),s.tokens[s.tokens.length-1].type=t._implements;do Cn(),a(t.lessThan)&&je();while(f(t.comma));C(n)}}function Xi(){a(t.lessThan)&&(ue(),a(t.parenL)||S())}function si(){let e=N(0);f(t.question),a(t.colon)&&Ie(),C(e)}function d1(){if(a(t._typeof)||_(l._type)){let e=xe();(jl(e)||e.type===t.braceL||e.type===t.star)&&d()}}function k1(){let e=s.contextualKeyword===l._type||s.type===t._typeof;e?d():v(),_(l._as)&&!st(l._as)?(v(),e&&!a(t.name)&&!(s.type&t.IS_KEYWORD)||v()):(e&&(a(t.name)||s.type&t.IS_KEYWORD)&&v(),W(l._as)&&v())}function g1(){if(a(t.lessThan)){let e=N(0);ue(),C(e)}}function x1(){a(t.colon)&&Ie()}function Ji(){if(a(t.colon)){let e=s.noAnonFunctionType;s.noAnonFunctionType=!0,Ie(),s.noAnonFunctionType=e}}function Ki(e,n){if(a(t.lessThan)){let r=s.snapshot(),o=ce(e,n);if(s.error)s.restoreFromSnapshot(r),s.type=t.typeParameterStart;else return o;let i=N(0);if(ue(),C(i),o=ce(e,n),o)return!0;S()}return ce(e,n)}function Qi(){if(a(t.colon)){let e=N(0),n=s.snapshot(),r=s.noAnonFunctionType;s.noAnonFunctionType=!0,Ks(),s.noAnonFunctionType=r,J()&&S(),a(t.arrow)||S(),s.error&&s.restoreFromSnapshot(n),C(e)}return f(t.arrow)}function Yi(e,n=!1){if(s.tokens[s.tokens.length-1].contextualKeyword===l._async&&a(t.lessThan)){let r=s.snapshot();if(su()&&!s.error)return;s.restoreFromSnapshot(r)}Vs(e,n)}function su(){s.scopeDepth++;let e=s.tokens.length;return be(),Tn()?(ht(e),!0):!1}function y1(){U(l._enum),s.tokens[s.tokens.length-1].type=t._enum,v(),ou()}function ou(){W(l._of)&&d(),m(t.braceL),iu(),m(t.braceR)}function iu(){for(;!a(t.braceR)&&!s.error&&!f(t.ellipsis);)au(),a(t.braceR)||m(t.comma)}function au(){v(),f(t.eq)&&d()}function I1(){if(pt(t.eof),s.scopes.push(new ee(0,s.tokens.length,!0)),s.scopeDepth!==0)throw new Error(`Invalid scope depth at end of file: ${s.scopeDepth}`);return new Ln(s.tokens,s.scopes)}function Z(e){L&&i1()||(a(t.at)&&vn(),cu(e))}function cu(e){if(R&&vi())return;let n=s.type;switch(n){case t._break:case t._continue:uu();return;case t._debugger:pu();return;case t._do:fu();return;case t._for:hu();return;case t._function:if(B()===t.dot)break;e||S(),ku();return;case t._class:e||S(),Te(!0);return;case t._if:gu();return;case t._return:xu();return;case t._switch:yu();return;case t._throw:_u();return;case t._try:Tu();return;case t._let:case t._const:e||S();case t._var:Lt(n!==t._var);return;case t._while:Iu();return;case t.braceL:we();return;case t.semi:wu();return;case t._export:case t._import:{let i=B();if(i===t.parenL||i===t.dot)break;d(),n===t._import?so():ro();return}case t.name:if(s.contextualKeyword===l._async){let i=s.start,c=s.snapshot();if(d(),a(t._function)&&!J()){m(t._function),_e(i,!0);return}else s.restoreFromSnapshot(c)}else if(s.contextualKeyword===l._using&&!tn()&&B()===t.name){Lt(!0);return}else if(w1()){U(l._await),Lt(!0);return}default:break}let r=s.tokens.length;G();let o=null;if(s.tokens.length===r+1){let i=s.tokens[s.tokens.length-1];i.type===t.name&&(o=i.contextualKeyword)}if(o==null){M();return}f(t.colon)?vu():Su(o)}function w1(){if(!_(l._await))return!1;let e=s.snapshot();return d(),!_(l._using)||X()?(s.restoreFromSnapshot(e),!1):(d(),!a(t.name)||X()?(s.restoreFromSnapshot(e),!1):(s.restoreFromSnapshot(e),!0))}function vn(){for(;a(t.at);)v1()}function v1(){if(d(),f(t.parenL))G(),m(t.parenR);else{for(v();f(t.dot);)v();lu()}}function lu(){R?ji():Fs()}function Fs(){f(t.parenL)&&fe()}function uu(){d(),ae()||(v(),M())}function pu(){d(),M()}function fu(){d(),Z(!1),m(t._while),Dt(),f(t.semi)}function hu(){s.scopeDepth++;let e=s.tokens.length;du();let n=s.tokens.length;s.scopes.push(new ee(e,n,!1)),s.scopeDepth--}function mu(){return!(!_(l._using)||st(l._of))}function du(){d();let e=!1;if(_(l._await)&&(e=!0,d()),m(t.parenL),a(t.semi)){e&&S(),oo();return}let n=w1();if(n||a(t._var)||a(t._let)||a(t._const)||mu()){if(n&&U(l._await),d(),S1(!0,s.type!==t._var),a(t._in)||_(l._of)){_1(e);return}oo();return}if(G(!0),a(t._in)||_(l._of)){_1(e);return}e&&S(),oo()}function ku(){let e=s.start;d(),_e(e,!0)}function gu(){d(),Dt(),Z(!1),f(t._else)&&Z(!1)}function xu(){d(),ae()||(G(),M())}function yu(){d(),Dt(),s.scopeDepth++;let e=s.tokens.length;for(m(t.braceL);!a(t.braceR)&&!s.error;)if(a(t._case)||a(t._default)){let r=a(t._case);d(),r&&G(),m(t.colon)}else Z(!0);d();let n=s.tokens.length;s.scopes.push(new ee(e,n,!1)),s.scopeDepth--}function _u(){d(),G(),M()}function bu(){St(!0),R&&Le()}function Tu(){if(d(),we(),a(t._catch)){d();let e=null;if(a(t.parenL)&&(s.scopeDepth++,e=s.tokens.length,m(t.parenL),bu(),m(t.parenR)),we(),e!=null){let n=s.tokens.length;s.scopes.push(new ee(e,n,!1)),s.scopeDepth--}}f(t._finally)&&we()}function Lt(e){d(),S1(!1,e),M()}function Iu(){d(),Dt(),Z(!1)}function wu(){d()}function vu(){Z(!0)}function Su(e){R?Ei(e):L?c1(e):M()}function we(e=!1,n=0){let r=s.tokens.length;s.scopeDepth++,m(t.braceL),n&&(s.tokens[s.tokens.length-1].contextId=n),pt(t.braceR),n&&(s.tokens[s.tokens.length-1].contextId=n);let o=s.tokens.length;s.scopes.push(new ee(r,o,e)),s.scopeDepth--}function pt(e){for(;!f(e)&&!s.error;)Z(!0)}function oo(){m(t.semi),a(t.semi)||G(),m(t.semi),a(t.parenR)||G(),m(t.parenR),Z(!1)}function _1(e){e?W(l._of):d(),G(),m(t.parenR),Z(!1)}function S1(e,n){for(;;){if(Eu(n),f(t.eq)){let r=s.tokens.length-1;z(e),s.tokens[r].rhsEndIndex=s.tokens.length}if(!f(t.comma))break}}function Eu(e){St(e),R?Ri():L&&x1()}function _e(e,n,r=!1){a(t.star)&&d(),n&&!r&&!a(t.name)&&!a(t._yield)&&S();let o=null;a(t.name)&&(n||(o=s.tokens.length,s.scopeDepth++),pe(!1));let i=s.tokens.length;s.scopeDepth++,be(),Gs(e);let c=s.tokens.length;s.scopes.push(new ee(i,c,!0)),s.scopeDepth--,o!==null&&(s.scopes.push(new ee(o,c,!0)),s.scopeDepth--)}function be(e=!1,n=0){R?Ni():L&&g1(),m(t.parenL),n&&(s.tokens[s.tokens.length-1].contextId=n),Et(t.parenR,!1,!1,e,n),n&&(s.tokens[s.tokens.length-1].contextId=n)}function Te(e,n=!1){let r=Ne();d(),s.tokens[s.tokens.length-1].contextId=r,s.tokens[s.tokens.length-1].isExpression=!e;let o=null;e||(o=s.tokens.length,s.scopeDepth++),Nu(e,n),Ru();let i=s.tokens.length;if(Au(r),!s.error&&(s.tokens[i].contextId=r,s.tokens[s.tokens.length-1].contextId=r,o!==null)){let c=s.tokens.length;s.scopes.push(new ee(o,c,!1)),s.scopeDepth--}}function E1(){return a(t.eq)||a(t.semi)||a(t.braceR)||a(t.bang)||a(t.colon)}function A1(){return a(t.parenL)||a(t.lessThan)}function Au(e){for(m(t.braceL);!f(t.braceR)&&!s.error;){if(f(t.semi))continue;if(a(t.at)){v1();continue}let n=s.start;Pu(n,e)}}function Pu(e,n){R&&At([l._declare,l._public,l._protected,l._private,l._override]);let r=!1;if(a(t.name)&&s.contextualKeyword===l._static){if(v(),A1()){jt(e,!1);return}else if(E1()){Rn();return}if(s.tokens[s.tokens.length-1].type=t._static,r=!0,a(t.braceL)){s.tokens[s.tokens.length-1].contextId=n,we();return}}Cu(e,r,n)}function Cu(e,n,r){if(R&&Si(n))return;if(f(t.star)){Ot(r),jt(e,!1);return}Ot(r);let o=!1,i=s.tokens[s.tokens.length-1];i.contextualKeyword===l._constructor&&(o=!0),b1(),A1()?jt(e,o):E1()?Rn():i.contextualKeyword===l._async&&!ae()?(s.tokens[s.tokens.length-1].type=t._async,a(t.star)&&d(),Ot(r),b1(),jt(e,!1)):(i.contextualKeyword===l._get||i.contextualKeyword===l._set)&&!(ae()&&a(t.star))?(i.contextualKeyword===l._get?s.tokens[s.tokens.length-1].type=t._get:s.tokens[s.tokens.length-1].type=t._set,Ot(r),jt(e,!1)):i.contextualKeyword===l._accessor&&!ae()?(Ot(r),Rn()):ae()?Rn():S()}function jt(e,n){R?Ce():L&&a(t.lessThan)&&ue(),In(e,n)}function Ot(e){De(e)}function b1(){if(R){let e=N(0);f(t.question),C(e)}}function Rn(){if(R?(sn(t.bang),Le()):L&&a(t.colon)&&Ie(),a(t.eq)){let e=s.tokens.length;d(),z(),s.tokens[e].rhsEndIndex=s.tokens.length}M()}function Nu(e,n=!1){R&&(!e||n)&&_(l._implements)||(a(t.name)&&pe(!0),R?Ce():L&&a(t.lessThan)&&ue())}function Ru(){let e=!1;f(t._extends)?(Us(),e=!0):e=!1,R?Pi(e):L&&m1(e)}function ro(){let e=s.tokens.length-1;R&&bi()||(ju()?Fu():Ou()?(v(),a(t.comma)&&B()===t.star?(m(t.comma),m(t.star),U(l._as),v()):P1(),mt()):f(t._default)?Lu():Bu()?Du():(Nn(),mt()),s.tokens[e].rhsEndIndex=s.tokens.length)}function Lu(){if(R&&wi()||L&&a1())return;let e=s.start;f(t._function)?_e(e,!0,!0):_(l._async)&&B()===t._function?(W(l._async),f(t._function),_e(e,!0,!0)):a(t._class)?Te(!0,!0):a(t.at)?(vn(),Te(!0,!0)):(z(),M())}function Du(){R?Ai():L?p1():Z(!0)}function Ou(){if(R&&Os())return!1;if(L&&u1())return!1;if(a(t.name))return s.contextualKeyword!==l._async;if(!a(t._default))return!1;let e=gt(),n=xe(),r=n.type===t.name&&n.contextualKeyword===l._from;if(n.type===t.comma)return!0;if(r){let o=g.charCodeAt(os(e+4));return o===u.quotationMark||o===u.apostrophe}return!1}function P1(){f(t.comma)&&Nn()}function mt(){W(l._from)&&(le(),C1()),M()}function ju(){return L?f1():a(t.star)}function Fu(){L?h1():Pn()}function Pn(){m(t.star),_(l._as)?Mu():mt()}function Mu(){d(),s.tokens[s.tokens.length-1].type=t._as,v(),P1(),mt()}function Bu(){return R&&Os()||L&&l1()||s.type===t._var||s.type===t._const||s.type===t._let||s.type===t._function||s.type===t._class||_(l._async)||a(t.at)}function Nn(){let e=!0;for(m(t.braceL);!f(t.braceR)&&!s.error;){if(e)e=!1;else if(m(t.comma),f(t.braceR))break;qu()}}function qu(){if(R){Ii();return}v(),s.tokens[s.tokens.length-1].identifierRole=b.ExportAccess,W(l._as)&&v()}function $u(){let e=s.snapshot();return U(l._module),W(l._from)?_(l._from)?(s.restoreFromSnapshot(e),!0):(s.restoreFromSnapshot(e),!1):a(t.comma)?(s.restoreFromSnapshot(e),!1):(s.restoreFromSnapshot(e),!0)}function Uu(){_(l._module)&&$u()&&d()}function so(){if(R&&a(t.name)&&B()===t.eq){gn();return}if(R&&_(l._type)){let e=xe();if(e.type===t.name&&e.contextualKeyword!==l._from){if(U(l._type),B()===t.eq){gn();return}}else(e.type===t.star||e.type===t.braceL)&&U(l._type)}a(t.string)?le():(Uu(),Vu(),U(l._from),le()),C1(),M()}function Hu(){return a(t.name)}function T1(){it()}function Vu(){L&&d1();let e=!0;if(!(Hu()&&(T1(),!f(t.comma)))){if(a(t.star)){d(),U(l._as),T1();return}for(m(t.braceL);!f(t.braceR)&&!s.error;){if(e)e=!1;else if(f(t.colon)&&S("ES2015 named imports do not destructure. Use another statement for destructuring after the import."),m(t.comma),f(t.braceR))break;Wu()}}}function Wu(){if(R){Ti();return}if(L){k1();return}it(),_(l._as)&&(s.tokens[s.tokens.length-1].identifierRole=b.ImportAccess,d(),it())}function C1(){(a(t._with)||_(l._assert)&&!X())&&(d(),Pt(!1,!1))}function N1(){return s.pos===0&&g.charCodeAt(0)===u.numberSign&&g.charCodeAt(1)===u.exclamationMark&&cs(2),as(),I1()}var Ln=class{constructor(n,r){this.tokens=n,this.scopes=r}};function R1(e,n,r,o){if(o&&r)throw new Error("Cannot combine flow and typescript plugins.");mo(e,n,r,o);let i=N1();if(s.error)throw ho(s.error);return i}function io(e){let n=e.currentIndex(),r=0,o=e.currentToken();do{let i=e.tokens[n];if(i.isOptionalChainStart&&r++,i.isOptionalChainEnd&&r--,r+=i.numNullishCoalesceStarts,r-=i.numNullishCoalesceEnds,i.contextualKeyword===l._await&&i.identifierRole==null&&i.scopeDepth===o.scopeDepth)return!0;n+=1}while(r>0&&n<e.tokens.length);return!1}var Ft=class e{__init(){this.resultCode=""}__init2(){this.resultMappings=new Array(this.tokens.length)}__init3(){this.tokenIndex=0}constructor(n,r,o,i,c){this.code=n,this.tokens=r,this.isFlowEnabled=o,this.disableESTransforms=i,this.helperManager=c,e.prototype.__init.call(this),e.prototype.__init2.call(this),e.prototype.__init3.call(this)}snapshot(){return{resultCode:this.resultCode,tokenIndex:this.tokenIndex}}restoreToSnapshot(n){this.resultCode=n.resultCode,this.tokenIndex=n.tokenIndex}dangerouslyGetAndRemoveCodeSinceSnapshot(n){let r=this.resultCode.slice(n.resultCode.length);return this.resultCode=n.resultCode,r}reset(){this.resultCode="",this.resultMappings=new Array(this.tokens.length),this.tokenIndex=0}matchesContextualAtIndex(n,r){return this.matches1AtIndex(n,t.name)&&this.tokens[n].contextualKeyword===r}identifierNameAtIndex(n){return this.identifierNameForToken(this.tokens[n])}identifierNameAtRelativeIndex(n){return this.identifierNameForToken(this.tokenAtRelativeIndex(n))}identifierName(){return this.identifierNameForToken(this.currentToken())}identifierNameForToken(n){return this.code.slice(n.start,n.end)}rawCodeForToken(n){return this.code.slice(n.start,n.end)}stringValueAtIndex(n){return this.stringValueForToken(this.tokens[n])}stringValue(){return this.stringValueForToken(this.currentToken())}stringValueForToken(n){return this.code.slice(n.start+1,n.end-1)}matches1AtIndex(n,r){return this.tokens[n].type===r}matches2AtIndex(n,r,o){return this.tokens[n].type===r&&this.tokens[n+1].type===o}matches3AtIndex(n,r,o,i){return this.tokens[n].type===r&&this.tokens[n+1].type===o&&this.tokens[n+2].type===i}matches1(n){return this.tokens[this.tokenIndex].type===n}matches2(n,r){return this.tokens[this.tokenIndex].type===n&&this.tokens[this.tokenIndex+1].type===r}matches3(n,r,o){return this.tokens[this.tokenIndex].type===n&&this.tokens[this.tokenIndex+1].type===r&&this.tokens[this.tokenIndex+2].type===o}matches4(n,r,o,i){return this.tokens[this.tokenIndex].type===n&&this.tokens[this.tokenIndex+1].type===r&&this.tokens[this.tokenIndex+2].type===o&&this.tokens[this.tokenIndex+3].type===i}matches5(n,r,o,i,c){return this.tokens[this.tokenIndex].type===n&&this.tokens[this.tokenIndex+1].type===r&&this.tokens[this.tokenIndex+2].type===o&&this.tokens[this.tokenIndex+3].type===i&&this.tokens[this.tokenIndex+4].type===c}matchesContextual(n){return this.matchesContextualAtIndex(this.tokenIndex,n)}matchesContextIdAndLabel(n,r){return this.matches1(n)&&this.currentToken().contextId===r}previousWhitespaceAndComments(){let n=this.code.slice(this.tokenIndex>0?this.tokens[this.tokenIndex-1].end:0,this.tokenIndex<this.tokens.length?this.tokens[this.tokenIndex].start:this.code.length);return this.isFlowEnabled&&(n=n.replace(/@flow/g,"")),n}replaceToken(n){this.resultCode+=this.previousWhitespaceAndComments(),this.appendTokenPrefix(),this.resultMappings[this.tokenIndex]=this.resultCode.length,this.resultCode+=n,this.appendTokenSuffix(),this.tokenIndex++}replaceTokenTrimmingLeftWhitespace(n){this.resultCode+=this.previousWhitespaceAndComments().replace(/[^\r\n]/g,""),this.appendTokenPrefix(),this.resultMappings[this.tokenIndex]=this.resultCode.length,this.resultCode+=n,this.appendTokenSuffix(),this.tokenIndex++}removeInitialToken(){this.replaceToken("")}removeToken(){this.replaceTokenTrimmingLeftWhitespace("")}removeBalancedCode(){let n=0;for(;!this.isAtEnd();){if(this.matches1(t.braceL))n++;else if(this.matches1(t.braceR)){if(n===0)return;n--}this.removeToken()}}copyExpectedToken(n){if(this.tokens[this.tokenIndex].type!==n)throw new Error(`Expected token ${n}`);this.copyToken()}copyToken(){this.resultCode+=this.previousWhitespaceAndComments(),this.appendTokenPrefix(),this.resultMappings[this.tokenIndex]=this.resultCode.length,this.resultCode+=this.code.slice(this.tokens[this.tokenIndex].start,this.tokens[this.tokenIndex].end),this.appendTokenSuffix(),this.tokenIndex++}copyTokenWithPrefix(n){this.resultCode+=this.previousWhitespaceAndComments(),this.appendTokenPrefix(),this.resultCode+=n,this.resultMappings[this.tokenIndex]=this.resultCode.length,this.resultCode+=this.code.slice(this.tokens[this.tokenIndex].start,this.tokens[this.tokenIndex].end),this.appendTokenSuffix(),this.tokenIndex++}appendTokenPrefix(){let n=this.currentToken();if((n.numNullishCoalesceStarts||n.isOptionalChainStart)&&(n.isAsyncOperation=io(this)),!this.disableESTransforms){if(n.numNullishCoalesceStarts)for(let r=0;r<n.numNullishCoalesceStarts;r++)n.isAsyncOperation?(this.resultCode+="await ",this.resultCode+=this.helperManager.getHelperName("asyncNullishCoalesce")):this.resultCode+=this.helperManager.getHelperName("nullishCoalesce"),this.resultCode+="(";n.isOptionalChainStart&&(n.isAsyncOperation&&(this.resultCode+="await "),this.tokenIndex>0&&this.tokenAtRelativeIndex(-1).type===t._delete?n.isAsyncOperation?this.resultCode+=this.helperManager.getHelperName("asyncOptionalChainDelete"):this.resultCode+=this.helperManager.getHelperName("optionalChainDelete"):n.isAsyncOperation?this.resultCode+=this.helperManager.getHelperName("asyncOptionalChain"):this.resultCode+=this.helperManager.getHelperName("optionalChain"),this.resultCode+="([")}}appendTokenSuffix(){let n=this.currentToken();if(n.isOptionalChainEnd&&!this.disableESTransforms&&(this.resultCode+="])"),n.numNullishCoalesceEnds&&!this.disableESTransforms)for(let r=0;r<n.numNullishCoalesceEnds;r++)this.resultCode+="))"}appendCode(n){this.resultCode+=n}currentToken(){return this.tokens[this.tokenIndex]}currentTokenCode(){let n=this.currentToken();return this.code.slice(n.start,n.end)}tokenAtRelativeIndex(n){return this.tokens[this.tokenIndex+n]}currentIndex(){return this.tokenIndex}nextToken(){if(this.tokenIndex===this.tokens.length)throw new Error("Unexpectedly reached end of input.");this.tokenIndex++}previousToken(){this.tokenIndex--}finish(){if(this.tokenIndex!==this.tokens.length)throw new Error("Tried to finish processing tokens before reaching the end.");return this.resultCode+=this.previousWhitespaceAndComments(),{code:this.resultCode,mappings:this.resultMappings}}isAtEnd(){return this.tokenIndex===this.tokens.length}};function co(e,n,r,o){let i=n.snapshot(),c=zu(n),p=[],h=[],x=[],k=null,y=[],w=[],I=n.currentToken().contextId;if(I==null)throw new Error("Expected non-null class context ID on class open-brace.");for(n.nextToken();!n.matchesContextIdAndLabel(t.braceR,I);)if(n.matchesContextual(l._constructor)&&!n.currentToken().isType)({constructorInitializerStatements:p,constructorInsertPos:k}=L1(n));else if(n.matches1(t.semi))o||w.push({start:n.currentIndex(),end:n.currentIndex()+1}),n.nextToken();else if(n.currentToken().isType)n.nextToken();else{let A=n.currentIndex(),O=!1,E=!1,$=!1;for(;Dn(n.currentToken());)n.matches1(t._static)&&(O=!0),n.matches1(t.hash)&&(E=!0),(n.matches1(t._declare)||n.matches1(t._abstract))&&($=!0),n.nextToken();if(O&&n.matches1(t.braceL)){ao(n,I);continue}if(E){ao(n,I);continue}if(n.matchesContextual(l._constructor)&&!n.currentToken().isType){({constructorInitializerStatements:p,constructorInsertPos:k}=L1(n));continue}let Q=n.currentIndex();if(Gu(n),n.matches1(t.lessThan)||n.matches1(t.parenL)){ao(n,I);continue}for(;n.currentToken().isType;)n.nextToken();if(n.matches1(t.eq)){let Se=n.currentIndex(),ge=n.currentToken().rhsEndIndex;if(ge==null)throw new Error("Expected rhsEndIndex on class field assignment.");for(n.nextToken();n.currentIndex()<ge;)e.processToken();let ie;O?(ie=r.claimFreeName("__initStatic"),x.push(ie)):(ie=r.claimFreeName("__init"),h.push(ie)),y.push({initializerName:ie,equalsIndex:Se,start:Q,end:n.currentIndex()})}else(!o||$)&&w.push({start:A,end:n.currentIndex()})}return n.restoreToSnapshot(i),o?{headerInfo:c,constructorInitializerStatements:p,instanceInitializerNames:[],staticInitializerNames:[],constructorInsertPos:k,fields:[],rangesToRemove:w}:{headerInfo:c,constructorInitializerStatements:p,instanceInitializerNames:h,staticInitializerNames:x,constructorInsertPos:k,fields:y,rangesToRemove:w}}function ao(e,n){for(e.nextToken();e.currentToken().contextId!==n;)e.nextToken();for(;Dn(e.tokenAtRelativeIndex(-1));)e.previousToken()}function zu(e){let n=e.currentToken(),r=n.contextId;if(r==null)throw new Error("Expected context ID on class token.");let o=n.isExpression;if(o==null)throw new Error("Expected isExpression on class token.");let i=null,c=!1;for(e.nextToken(),e.matches1(t.name)&&(i=e.identifierName());!e.matchesContextIdAndLabel(t.braceL,r);)e.matches1(t._extends)&&!e.currentToken().isType&&(c=!0),e.nextToken();return{isExpression:o,className:i,hasSuperclass:c}}function L1(e){let n=[];e.nextToken();let r=e.currentToken().contextId;if(r==null)throw new Error("Expected context ID on open-paren starting constructor params.");for(;!e.matchesContextIdAndLabel(t.parenR,r);)if(e.currentToken().contextId===r){if(e.nextToken(),Dn(e.currentToken())){for(e.nextToken();Dn(e.currentToken());)e.nextToken();let c=e.currentToken();if(c.type!==t.name)throw new Error("Expected identifier after access modifiers in constructor arg.");let p=e.identifierNameForToken(c);n.push(`this.${p} = ${p}`)}}else e.nextToken();for(e.nextToken();e.currentToken().isType;)e.nextToken();let o=e.currentIndex(),i=!1;for(;!e.matchesContextIdAndLabel(t.braceR,r);){if(!i&&e.matches2(t._super,t.parenL)){e.nextToken();let c=e.currentToken().contextId;if(c==null)throw new Error("Expected a context ID on the super call");for(;!e.matchesContextIdAndLabel(t.parenR,c);)e.nextToken();o=e.currentIndex(),i=!0}e.nextToken()}return e.nextToken(),{constructorInitializerStatements:n,constructorInsertPos:o}}function Dn(e){return[t._async,t._get,t._set,t.plus,t.minus,t._readonly,t._static,t._public,t._private,t._protected,t._override,t._abstract,t.star,t._declare,t.hash].includes(e.type)}function Gu(e){if(e.matches1(t.bracketL)){let r=e.currentToken().contextId;if(r==null)throw new Error("Expected class context ID on computed name open bracket.");for(;!e.matchesContextIdAndLabel(t.bracketR,r);)e.nextToken();e.nextToken()}else e.nextToken()}function Mt(e){if(e.removeInitialToken(),e.removeToken(),e.removeToken(),e.removeToken(),e.matches1(t.parenL))e.removeToken(),e.removeToken(),e.removeToken();else for(;e.matches1(t.dot);)e.removeToken(),e.removeToken()}var On={typeDeclarations:new Set,valueDeclarations:new Set};function Bt(e){let n=new Set,r=new Set;for(let o=0;o<e.tokens.length;o++){let i=e.tokens[o];i.type===t.name&&rn(i)&&(i.isType?n.add(e.identifierNameForToken(i)):r.add(e.identifierNameForToken(i)))}return{typeDeclarations:n,valueDeclarations:r}}function qt(e){let n=e.currentIndex();for(;!e.matches1AtIndex(n,t.braceR);)n++;return e.matchesContextualAtIndex(n+1,l._from)&&e.matches1AtIndex(n+2,t.string)}function ve(e){(e.matches2(t._with,t.braceL)||e.matches2(t.name,t.braceL)&&e.matchesContextual(l._assert))&&(e.removeToken(),e.removeToken(),e.removeBalancedCode(),e.removeToken())}function $t(e,n,r,o){if(!e||n)return!1;let i=r.currentToken();if(i.rhsEndIndex==null)throw new Error("Expected non-null rhsEndIndex on export token.");let c=i.rhsEndIndex-r.currentIndex();if(c!==3&&!(c===4&&r.matches1AtIndex(i.rhsEndIndex-1,t.semi)))return!1;let p=r.tokenAtRelativeIndex(2);if(p.type!==t.name)return!1;let h=r.identifierNameForToken(p);return o.typeDeclarations.has(h)&&!o.valueDeclarations.has(h)}var Ut=class e extends H{__init(){this.hadExport=!1}__init2(){this.hadNamedExport=!1}__init3(){this.hadDefaultExport=!1}constructor(n,r,o,i,c,p,h,x,k,y,w,I){super(),this.rootTransformer=n,this.tokens=r,this.importProcessor=o,this.nameManager=i,this.helperManager=c,this.reactHotLoaderTransformer=p,this.enableLegacyBabel5ModuleInterop=h,this.enableLegacyTypeScriptModuleInterop=x,this.isTypeScriptTransformEnabled=k,this.isFlowTransformEnabled=y,this.preserveDynamicImport=w,this.keepUnusedImports=I,e.prototype.__init.call(this),e.prototype.__init2.call(this),e.prototype.__init3.call(this),this.declarationInfo=k?Bt(r):On}getPrefixCode(){let n="";return this.hadExport&&(n+='Object.defineProperty(exports, "__esModule", {value: true});'),n}getSuffixCode(){return this.enableLegacyBabel5ModuleInterop&&this.hadDefaultExport&&!this.hadNamedExport?`
module.exports = exports.default;
`:""}process(){return this.tokens.matches3(t._import,t.name,t.eq)?this.processImportEquals():this.tokens.matches1(t._import)?(this.processImport(),!0):this.tokens.matches2(t._export,t.eq)?(this.tokens.replaceToken("module.exports"),!0):this.tokens.matches1(t._export)&&!this.tokens.currentToken().isType?(this.hadExport=!0,this.processExport()):this.tokens.matches2(t.name,t.postIncDec)&&this.processPostIncDec()?!0:this.tokens.matches1(t.name)||this.tokens.matches1(t.jsxName)?this.processIdentifier():this.tokens.matches1(t.eq)?this.processAssignment():this.tokens.matches1(t.assign)?this.processComplexAssignment():this.tokens.matches1(t.preIncDec)?this.processPreIncDec():!1}processImportEquals(){let n=this.tokens.identifierNameAtIndex(this.tokens.currentIndex()+1);return this.importProcessor.shouldAutomaticallyElideImportedName(n)?Mt(this.tokens):this.tokens.replaceToken("const"),!0}processImport(){if(this.tokens.matches2(t._import,t.parenL)){if(this.preserveDynamicImport){this.tokens.copyToken();return}let r=this.enableLegacyTypeScriptModuleInterop?"":`${this.helperManager.getHelperName("interopRequireWildcard")}(`;this.tokens.replaceToken(`Promise.resolve().then(() => ${r}require`);let o=this.tokens.currentToken().contextId;if(o==null)throw new Error("Expected context ID on dynamic import invocation.");for(this.tokens.copyToken();!this.tokens.matchesContextIdAndLabel(t.parenR,o);)this.rootTransformer.processToken();this.tokens.replaceToken(r?")))":"))");return}if(this.removeImportAndDetectIfShouldElide())this.tokens.removeToken();else{let r=this.tokens.stringValue();this.tokens.replaceTokenTrimmingLeftWhitespace(this.importProcessor.claimImportCode(r)),this.tokens.appendCode(this.importProcessor.claimImportCode(r))}ve(this.tokens),this.tokens.matches1(t.semi)&&this.tokens.removeToken()}removeImportAndDetectIfShouldElide(){if(this.tokens.removeInitialToken(),this.tokens.matchesContextual(l._type)&&!this.tokens.matches1AtIndex(this.tokens.currentIndex()+1,t.comma)&&!this.tokens.matchesContextualAtIndex(this.tokens.currentIndex()+1,l._from))return this.removeRemainingImport(),!0;if(this.tokens.matches1(t.name)||this.tokens.matches1(t.star))return this.removeRemainingImport(),!1;if(this.tokens.matches1(t.string))return!1;let n=!1,r=!1;for(;!this.tokens.matches1(t.string);)(!n&&this.tokens.matches1(t.braceL)||this.tokens.matches1(t.comma))&&(this.tokens.removeToken(),this.tokens.matches1(t.braceR)||(r=!0),(this.tokens.matches2(t.name,t.comma)||this.tokens.matches2(t.name,t.braceR)||this.tokens.matches4(t.name,t.name,t.name,t.comma)||this.tokens.matches4(t.name,t.name,t.name,t.braceR))&&(n=!0)),this.tokens.removeToken();return this.keepUnusedImports?!1:this.isTypeScriptTransformEnabled?!n:this.isFlowTransformEnabled?r&&!n:!1}removeRemainingImport(){for(;!this.tokens.matches1(t.string);)this.tokens.removeToken()}processIdentifier(){let n=this.tokens.currentToken();if(n.shadowsGlobal)return!1;if(n.identifierRole===b.ObjectShorthand)return this.processObjectShorthand();if(n.identifierRole!==b.Access)return!1;let r=this.importProcessor.getIdentifierReplacement(this.tokens.identifierNameForToken(n));if(!r)return!1;let o=this.tokens.currentIndex()+1;for(;o<this.tokens.tokens.length&&this.tokens.tokens[o].type===t.parenR;)o++;return this.tokens.tokens[o].type===t.parenL?this.tokens.tokenAtRelativeIndex(1).type===t.parenL&&this.tokens.tokenAtRelativeIndex(-1).type!==t._new?(this.tokens.replaceToken(`${r}.call(void 0, `),this.tokens.removeToken(),this.rootTransformer.processBalancedCode(),this.tokens.copyExpectedToken(t.parenR)):this.tokens.replaceToken(`(0, ${r})`):this.tokens.replaceToken(r),!0}processObjectShorthand(){let n=this.tokens.identifierName(),r=this.importProcessor.getIdentifierReplacement(n);return r?(this.tokens.replaceToken(`${n}: ${r}`),!0):!1}processExport(){if(this.tokens.matches2(t._export,t._enum)||this.tokens.matches3(t._export,t._const,t._enum))return this.hadNamedExport=!0,!1;if(this.tokens.matches2(t._export,t._default))return this.tokens.matches3(t._export,t._default,t._enum)?(this.hadDefaultExport=!0,!1):(this.processExportDefault(),!0);if(this.tokens.matches2(t._export,t.braceL))return this.processExportBindings(),!0;if(this.tokens.matches2(t._export,t.name)&&this.tokens.matchesContextualAtIndex(this.tokens.currentIndex()+1,l._type)){if(this.tokens.removeInitialToken(),this.tokens.removeToken(),this.tokens.matches1(t.braceL)){for(;!this.tokens.matches1(t.braceR);)this.tokens.removeToken();this.tokens.removeToken()}else this.tokens.removeToken(),this.tokens.matches1(t._as)&&(this.tokens.removeToken(),this.tokens.removeToken());return this.tokens.matchesContextual(l._from)&&this.tokens.matches1AtIndex(this.tokens.currentIndex()+1,t.string)&&(this.tokens.removeToken(),this.tokens.removeToken(),ve(this.tokens)),!0}if(this.hadNamedExport=!0,this.tokens.matches2(t._export,t._var)||this.tokens.matches2(t._export,t._let)||this.tokens.matches2(t._export,t._const))return this.processExportVar(),!0;if(this.tokens.matches2(t._export,t._function)||this.tokens.matches3(t._export,t.name,t._function))return this.processExportFunction(),!0;if(this.tokens.matches2(t._export,t._class)||this.tokens.matches3(t._export,t._abstract,t._class)||this.tokens.matches2(t._export,t.at))return this.processExportClass(),!0;if(this.tokens.matches2(t._export,t.star))return this.processExportStar(),!0;throw new Error("Unrecognized export syntax.")}processAssignment(){let n=this.tokens.currentIndex(),r=this.tokens.tokens[n-1];if(r.isType||r.type!==t.name||r.shadowsGlobal||n>=2&&this.tokens.matches1AtIndex(n-2,t.dot)||n>=2&&[t._var,t._let,t._const].includes(this.tokens.tokens[n-2].type))return!1;let o=this.importProcessor.resolveExportBinding(this.tokens.identifierNameForToken(r));return o?(this.tokens.copyToken(),this.tokens.appendCode(` ${o} =`),!0):!1}processComplexAssignment(){let n=this.tokens.currentIndex(),r=this.tokens.tokens[n-1];if(r.type!==t.name||r.shadowsGlobal||n>=2&&this.tokens.matches1AtIndex(n-2,t.dot))return!1;let o=this.importProcessor.resolveExportBinding(this.tokens.identifierNameForToken(r));return o?(this.tokens.appendCode(` = ${o}`),this.tokens.copyToken(),!0):!1}processPreIncDec(){let n=this.tokens.currentIndex(),r=this.tokens.tokens[n+1];if(r.type!==t.name||r.shadowsGlobal||n+2<this.tokens.tokens.length&&(this.tokens.matches1AtIndex(n+2,t.dot)||this.tokens.matches1AtIndex(n+2,t.bracketL)||this.tokens.matches1AtIndex(n+2,t.parenL)))return!1;let o=this.tokens.identifierNameForToken(r),i=this.importProcessor.resolveExportBinding(o);return i?(this.tokens.appendCode(`${i} = `),this.tokens.copyToken(),!0):!1}processPostIncDec(){let n=this.tokens.currentIndex(),r=this.tokens.tokens[n],o=this.tokens.tokens[n+1];if(r.type!==t.name||r.shadowsGlobal||n>=1&&this.tokens.matches1AtIndex(n-1,t.dot))return!1;let i=this.tokens.identifierNameForToken(r),c=this.importProcessor.resolveExportBinding(i);if(!c)return!1;let p=this.tokens.rawCodeForToken(o),h=this.importProcessor.getIdentifierReplacement(i)||i;if(p==="++")this.tokens.replaceToken(`(${h} = ${c} = ${h} + 1, ${h} - 1)`);else if(p==="--")this.tokens.replaceToken(`(${h} = ${c} = ${h} - 1, ${h} + 1)`);else throw new Error(`Unexpected operator: ${p}`);return this.tokens.removeToken(),!0}processExportDefault(){let n=!0;if(this.tokens.matches4(t._export,t._default,t._function,t.name)||this.tokens.matches5(t._export,t._default,t.name,t._function,t.name)&&this.tokens.matchesContextualAtIndex(this.tokens.currentIndex()+2,l._async)){this.tokens.removeInitialToken(),this.tokens.removeToken();let r=this.processNamedFunction();this.tokens.appendCode(` exports.default = ${r};`)}else if(this.tokens.matches4(t._export,t._default,t._class,t.name)||this.tokens.matches5(t._export,t._default,t._abstract,t._class,t.name)||this.tokens.matches3(t._export,t._default,t.at)){this.tokens.removeInitialToken(),this.tokens.removeToken(),this.copyDecorators(),this.tokens.matches1(t._abstract)&&this.tokens.removeToken();let r=this.rootTransformer.processNamedClass();this.tokens.appendCode(` exports.default = ${r};`)}else if($t(this.isTypeScriptTransformEnabled,this.keepUnusedImports,this.tokens,this.declarationInfo))n=!1,this.tokens.removeInitialToken(),this.tokens.removeToken(),this.tokens.removeToken();else if(this.reactHotLoaderTransformer){let r=this.nameManager.claimFreeName("_default");this.tokens.replaceToken(`let ${r}; exports.`),this.tokens.copyToken(),this.tokens.appendCode(` = ${r} =`),this.reactHotLoaderTransformer.setExtractedDefaultExportName(r)}else this.tokens.replaceToken("exports."),this.tokens.copyToken(),this.tokens.appendCode(" =");n&&(this.hadDefaultExport=!0)}copyDecorators(){for(;this.tokens.matches1(t.at);)if(this.tokens.copyToken(),this.tokens.matches1(t.parenL))this.tokens.copyExpectedToken(t.parenL),this.rootTransformer.processBalancedCode(),this.tokens.copyExpectedToken(t.parenR);else{for(this.tokens.copyExpectedToken(t.name);this.tokens.matches1(t.dot);)this.tokens.copyExpectedToken(t.dot),this.tokens.copyExpectedToken(t.name);this.tokens.matches1(t.parenL)&&(this.tokens.copyExpectedToken(t.parenL),this.rootTransformer.processBalancedCode(),this.tokens.copyExpectedToken(t.parenR))}}processExportVar(){this.isSimpleExportVar()?this.processSimpleExportVar():this.processComplexExportVar()}isSimpleExportVar(){let n=this.tokens.currentIndex();if(n++,n++,!this.tokens.matches1AtIndex(n,t.name))return!1;for(n++;n<this.tokens.tokens.length&&this.tokens.tokens[n].isType;)n++;return!!this.tokens.matches1AtIndex(n,t.eq)}processSimpleExportVar(){this.tokens.removeInitialToken(),this.tokens.copyToken();let n=this.tokens.identifierName();for(;!this.tokens.matches1(t.eq);)this.rootTransformer.processToken();let r=this.tokens.currentToken().rhsEndIndex;if(r==null)throw new Error("Expected = token with an end index.");for(;this.tokens.currentIndex()<r;)this.rootTransformer.processToken();this.tokens.appendCode(`; exports.${n} = ${n}`)}processComplexExportVar(){this.tokens.removeInitialToken(),this.tokens.removeToken();let n=this.tokens.matches1(t.braceL);n&&this.tokens.appendCode("(");let r=0;for(;;)if(this.tokens.matches1(t.braceL)||this.tokens.matches1(t.dollarBraceL)||this.tokens.matches1(t.bracketL))r++,this.tokens.copyToken();else if(this.tokens.matches1(t.braceR)||this.tokens.matches1(t.bracketR))r--,this.tokens.copyToken();else{if(r===0&&!this.tokens.matches1(t.name)&&!this.tokens.currentToken().isType)break;if(this.tokens.matches1(t.eq)){let o=this.tokens.currentToken().rhsEndIndex;if(o==null)throw new Error("Expected = token with an end index.");for(;this.tokens.currentIndex()<o;)this.rootTransformer.processToken()}else{let o=this.tokens.currentToken();if(nn(o)){let i=this.tokens.identifierName(),c=this.importProcessor.getIdentifierReplacement(i);if(c===null)throw new Error(`Expected a replacement for ${i} in \`export var\` syntax.`);yo(o)&&(c=`${i}: ${c}`),this.tokens.replaceToken(c)}else this.rootTransformer.processToken()}}if(n){let o=this.tokens.currentToken().rhsEndIndex;if(o==null)throw new Error("Expected = token with an end index.");for(;this.tokens.currentIndex()<o;)this.rootTransformer.processToken();this.tokens.appendCode(")")}}processExportFunction(){this.tokens.replaceToken("");let n=this.processNamedFunction();this.tokens.appendCode(` exports.${n} = ${n};`)}processNamedFunction(){if(this.tokens.matches1(t._function))this.tokens.copyToken();else if(this.tokens.matches2(t.name,t._function)){if(!this.tokens.matchesContextual(l._async))throw new Error("Expected async keyword in function export.");this.tokens.copyToken(),this.tokens.copyToken()}if(this.tokens.matches1(t.star)&&this.tokens.copyToken(),!this.tokens.matches1(t.name))throw new Error("Expected identifier for exported function name.");let n=this.tokens.identifierName();if(this.tokens.copyToken(),this.tokens.currentToken().isType)for(this.tokens.removeInitialToken();this.tokens.currentToken().isType;)this.tokens.removeToken();return this.tokens.copyExpectedToken(t.parenL),this.rootTransformer.processBalancedCode(),this.tokens.copyExpectedToken(t.parenR),this.rootTransformer.processPossibleTypeRange(),this.tokens.copyExpectedToken(t.braceL),this.rootTransformer.processBalancedCode(),this.tokens.copyExpectedToken(t.braceR),n}processExportClass(){this.tokens.removeInitialToken(),this.copyDecorators(),this.tokens.matches1(t._abstract)&&this.tokens.removeToken();let n=this.rootTransformer.processNamedClass();this.tokens.appendCode(` exports.${n} = ${n};`)}processExportBindings(){this.tokens.removeInitialToken(),this.tokens.removeToken();let n=qt(this.tokens),r=[];for(;;){if(this.tokens.matches1(t.braceR)){this.tokens.removeToken();break}let o=me(this.tokens);for(;this.tokens.currentIndex()<o.endIndex;)this.tokens.removeToken();if(!(o.isType||!n&&this.shouldElideExportedIdentifier(o.leftName))){let c=o.rightName;c==="default"?this.hadDefaultExport=!0:this.hadNamedExport=!0;let p=o.leftName,h=this.importProcessor.getIdentifierReplacement(p);r.push(`exports.${c} = ${h||p};`)}if(this.tokens.matches1(t.braceR)){this.tokens.removeToken();break}if(this.tokens.matches2(t.comma,t.braceR)){this.tokens.removeToken(),this.tokens.removeToken();break}else if(this.tokens.matches1(t.comma))this.tokens.removeToken();else throw new Error(`Unexpected token: ${JSON.stringify(this.tokens.currentToken())}`)}if(this.tokens.matchesContextual(l._from)){this.tokens.removeToken();let o=this.tokens.stringValue();this.tokens.replaceTokenTrimmingLeftWhitespace(this.importProcessor.claimImportCode(o)),ve(this.tokens)}else this.tokens.appendCode(r.join(" "));this.tokens.matches1(t.semi)&&this.tokens.removeToken()}processExportStar(){for(this.tokens.removeInitialToken();!this.tokens.matches1(t.string);)this.tokens.removeToken();let n=this.tokens.stringValue();this.tokens.replaceTokenTrimmingLeftWhitespace(this.importProcessor.claimImportCode(n)),ve(this.tokens),this.tokens.matches1(t.semi)&&this.tokens.removeToken()}shouldElideExportedIdentifier(n){return this.isTypeScriptTransformEnabled&&!this.keepUnusedImports&&!this.declarationInfo.valueDeclarations.has(n)}};var Ht=class extends H{constructor(n,r,o,i,c,p,h,x){super(),this.tokens=n,this.nameManager=r,this.helperManager=o,this.reactHotLoaderTransformer=i,this.isTypeScriptTransformEnabled=c,this.isFlowTransformEnabled=p,this.keepUnusedImports=h,this.nonTypeIdentifiers=c&&!h?cn(n,x):new Set,this.declarationInfo=c&&!h?Bt(n):On,this.injectCreateRequireForImportRequire=!!x.injectCreateRequireForImportRequire}process(){if(this.tokens.matches3(t._import,t.name,t.eq))return this.processImportEquals();if(this.tokens.matches4(t._import,t.name,t.name,t.eq)&&this.tokens.matchesContextualAtIndex(this.tokens.currentIndex()+1,l._type)){this.tokens.removeInitialToken();for(let n=0;n<7;n++)this.tokens.removeToken();return!0}if(this.tokens.matches2(t._export,t.eq))return this.tokens.replaceToken("module.exports"),!0;if(this.tokens.matches5(t._export,t._import,t.name,t.name,t.eq)&&this.tokens.matchesContextualAtIndex(this.tokens.currentIndex()+2,l._type)){this.tokens.removeInitialToken();for(let n=0;n<8;n++)this.tokens.removeToken();return!0}if(this.tokens.matches1(t._import))return this.processImport();if(this.tokens.matches2(t._export,t._default))return this.processExportDefault();if(this.tokens.matches2(t._export,t.braceL))return this.processNamedExports();if(this.tokens.matches2(t._export,t.name)&&this.tokens.matchesContextualAtIndex(this.tokens.currentIndex()+1,l._type)){if(this.tokens.removeInitialToken(),this.tokens.removeToken(),this.tokens.matches1(t.braceL)){for(;!this.tokens.matches1(t.braceR);)this.tokens.removeToken();this.tokens.removeToken()}else this.tokens.removeToken(),this.tokens.matches1(t._as)&&(this.tokens.removeToken(),this.tokens.removeToken());return this.tokens.matchesContextual(l._from)&&this.tokens.matches1AtIndex(this.tokens.currentIndex()+1,t.string)&&(this.tokens.removeToken(),this.tokens.removeToken(),ve(this.tokens)),!0}return!1}processImportEquals(){let n=this.tokens.identifierNameAtIndex(this.tokens.currentIndex()+1);return this.shouldAutomaticallyElideImportedName(n)?Mt(this.tokens):this.injectCreateRequireForImportRequire?(this.tokens.replaceToken("const"),this.tokens.copyToken(),this.tokens.copyToken(),this.tokens.replaceToken(this.helperManager.getHelperName("require"))):this.tokens.replaceToken("const"),!0}processImport(){if(this.tokens.matches2(t._import,t.parenL))return!1;let n=this.tokens.snapshot();if(this.removeImportTypeBindings()){for(this.tokens.restoreToSnapshot(n);!this.tokens.matches1(t.string);)this.tokens.removeToken();this.tokens.removeToken(),ve(this.tokens),this.tokens.matches1(t.semi)&&this.tokens.removeToken()}return!0}removeImportTypeBindings(){if(this.tokens.copyExpectedToken(t._import),this.tokens.matchesContextual(l._type)&&!this.tokens.matches1AtIndex(this.tokens.currentIndex()+1,t.comma)&&!this.tokens.matchesContextualAtIndex(this.tokens.currentIndex()+1,l._from))return!0;if(this.tokens.matches1(t.string))return this.tokens.copyToken(),!1;this.tokens.matchesContextual(l._module)&&this.tokens.matchesContextualAtIndex(this.tokens.currentIndex()+2,l._from)&&this.tokens.copyToken();let n=!1,r=!1,o=!1;if(this.tokens.matches1(t.name)&&(this.shouldAutomaticallyElideImportedName(this.tokens.identifierName())?(this.tokens.removeToken(),this.tokens.matches1(t.comma)&&this.tokens.removeToken()):(n=!0,this.tokens.copyToken(),this.tokens.matches1(t.comma)&&(o=!0,this.tokens.removeToken()))),this.tokens.matches1(t.star))this.shouldAutomaticallyElideImportedName(this.tokens.identifierNameAtRelativeIndex(2))?(this.tokens.removeToken(),this.tokens.removeToken(),this.tokens.removeToken()):(o&&this.tokens.appendCode(","),n=!0,this.tokens.copyExpectedToken(t.star),this.tokens.copyExpectedToken(t.name),this.tokens.copyExpectedToken(t.name));else if(this.tokens.matches1(t.braceL)){for(o&&this.tokens.appendCode(","),this.tokens.copyToken();!this.tokens.matches1(t.braceR);){r=!0;let i=me(this.tokens);if(i.isType||this.shouldAutomaticallyElideImportedName(i.rightName)){for(;this.tokens.currentIndex()<i.endIndex;)this.tokens.removeToken();this.tokens.matches1(t.comma)&&this.tokens.removeToken()}else{for(n=!0;this.tokens.currentIndex()<i.endIndex;)this.tokens.copyToken();this.tokens.matches1(t.comma)&&this.tokens.copyToken()}}this.tokens.copyExpectedToken(t.braceR)}return this.keepUnusedImports?!1:this.isTypeScriptTransformEnabled?!n:this.isFlowTransformEnabled?r&&!n:!1}shouldAutomaticallyElideImportedName(n){return this.isTypeScriptTransformEnabled&&!this.keepUnusedImports&&!this.nonTypeIdentifiers.has(n)}processExportDefault(){if($t(this.isTypeScriptTransformEnabled,this.keepUnusedImports,this.tokens,this.declarationInfo))return this.tokens.removeInitialToken(),this.tokens.removeToken(),this.tokens.removeToken(),!0;if(!(this.tokens.matches4(t._export,t._default,t._function,t.name)||this.tokens.matches5(t._export,t._default,t.name,t._function,t.name)&&this.tokens.matchesContextualAtIndex(this.tokens.currentIndex()+2,l._async)||this.tokens.matches4(t._export,t._default,t._class,t.name)||this.tokens.matches5(t._export,t._default,t._abstract,t._class,t.name))&&this.reactHotLoaderTransformer){let r=this.nameManager.claimFreeName("_default");return this.tokens.replaceToken(`let ${r}; export`),this.tokens.copyToken(),this.tokens.appendCode(` ${r} =`),this.reactHotLoaderTransformer.setExtractedDefaultExportName(r),!0}return!1}processNamedExports(){if(!this.isTypeScriptTransformEnabled)return!1;this.tokens.copyExpectedToken(t._export),this.tokens.copyExpectedToken(t.braceL);let n=qt(this.tokens),r=!1;for(;!this.tokens.matches1(t.braceR);){let o=me(this.tokens);if(o.isType||!n&&this.shouldElideExportedName(o.leftName)){for(;this.tokens.currentIndex()<o.endIndex;)this.tokens.removeToken();this.tokens.matches1(t.comma)&&this.tokens.removeToken()}else{for(r=!0;this.tokens.currentIndex()<o.endIndex;)this.tokens.copyToken();this.tokens.matches1(t.comma)&&this.tokens.copyToken()}}return this.tokens.copyExpectedToken(t.braceR),!this.keepUnusedImports&&n&&!r&&(this.tokens.removeToken(),this.tokens.removeToken(),ve(this.tokens)),!0}shouldElideExportedName(n){return this.isTypeScriptTransformEnabled&&!this.keepUnusedImports&&this.declarationInfo.typeDeclarations.has(n)&&!this.declarationInfo.valueDeclarations.has(n)}};var Vt=class extends H{constructor(n,r,o){super(),this.rootTransformer=n,this.tokens=r,this.isImportsTransformEnabled=o}process(){return this.rootTransformer.processPossibleArrowParamEnd()||this.rootTransformer.processPossibleAsyncArrowWithTypeParams()||this.rootTransformer.processPossibleTypeRange()?!0:this.tokens.matches1(t._enum)?(this.processEnum(),!0):this.tokens.matches2(t._export,t._enum)?(this.processNamedExportEnum(),!0):this.tokens.matches3(t._export,t._default,t._enum)?(this.processDefaultExportEnum(),!0):!1}processNamedExportEnum(){if(this.isImportsTransformEnabled){this.tokens.removeInitialToken();let n=this.tokens.identifierNameAtRelativeIndex(1);this.processEnum(),this.tokens.appendCode(` exports.${n} = ${n};`)}else this.tokens.copyToken(),this.processEnum()}processDefaultExportEnum(){this.tokens.removeInitialToken(),this.tokens.removeToken();let n=this.tokens.identifierNameAtRelativeIndex(1);this.processEnum(),this.isImportsTransformEnabled?this.tokens.appendCode(` exports.default = ${n};`):this.tokens.appendCode(` export default ${n};`)}processEnum(){this.tokens.replaceToken("const"),this.tokens.copyExpectedToken(t.name);let n=!1;this.tokens.matchesContextual(l._of)&&(this.tokens.removeToken(),n=this.tokens.matchesContextual(l._symbol),this.tokens.removeToken());let r=this.tokens.matches3(t.braceL,t.name,t.eq);this.tokens.appendCode(' = require("flow-enums-runtime")');let o=!n&&!r;for(this.tokens.replaceTokenTrimmingLeftWhitespace(o?".Mirrored([":"({");!this.tokens.matches1(t.braceR);){if(this.tokens.matches1(t.ellipsis)){this.tokens.removeToken();break}this.processEnumElement(n,r),this.tokens.matches1(t.comma)&&this.tokens.copyToken()}this.tokens.replaceToken(o?"]);":"});")}processEnumElement(n,r){if(n){let o=this.tokens.identifierName();this.tokens.copyToken(),this.tokens.appendCode(`: Symbol("${o}")`)}else r?(this.tokens.copyToken(),this.tokens.replaceTokenTrimmingLeftWhitespace(":"),this.tokens.copyToken()):this.tokens.replaceToken(`"${this.tokens.identifierName()}"`)}};function Xu(e){let n,r=e[0],o=1;for(;o<e.length;){let i=e[o],c=e[o+1];if(o+=2,(i==="optionalAccess"||i==="optionalCall")&&r==null)return;i==="access"||i==="optionalAccess"?(n=r,r=c(r)):(i==="call"||i==="optionalCall")&&(r=c((...p)=>r.call(n,...p)),n=void 0)}return r}var jn="jest",Ju=["mock","unmock","enableAutomock","disableAutomock"],Wt=class e extends H{__init(){this.hoistedFunctionNames=[]}constructor(n,r,o,i){super(),this.rootTransformer=n,this.tokens=r,this.nameManager=o,this.importProcessor=i,e.prototype.__init.call(this)}process(){return this.tokens.currentToken().scopeDepth===0&&this.tokens.matches4(t.name,t.dot,t.name,t.parenL)&&this.tokens.identifierName()===jn?Xu([this,"access",n=>n.importProcessor,"optionalAccess",n=>n.getGlobalNames,"call",n=>n(),"optionalAccess",n=>n.has,"call",n=>n(jn)])?!1:this.extractHoistedCalls():!1}getHoistedCode(){return this.hoistedFunctionNames.length>0?this.hoistedFunctionNames.map(n=>`${n}();`).join(""):""}extractHoistedCalls(){this.tokens.removeToken();let n=!1;for(;this.tokens.matches3(t.dot,t.name,t.parenL);){let r=this.tokens.identifierNameAtIndex(this.tokens.currentIndex()+1);if(Ju.includes(r)){let i=this.nameManager.claimFreeName("__jestHoist");this.hoistedFunctionNames.push(i),this.tokens.replaceToken(`function ${i}(){${jn}.`),this.tokens.copyToken(),this.tokens.copyToken(),this.rootTransformer.processBalancedCode(),this.tokens.copyExpectedToken(t.parenR),this.tokens.appendCode(";}"),n=!1}else n?this.tokens.copyToken():this.tokens.replaceToken(`${jn}.`),this.tokens.copyToken(),this.tokens.copyToken(),this.rootTransformer.processBalancedCode(),this.tokens.copyExpectedToken(t.parenR),n=!0}return!0}};var zt=class extends H{constructor(n){super(),this.tokens=n}process(){if(this.tokens.matches1(t.num)){let n=this.tokens.currentTokenCode();if(n.includes("_"))return this.tokens.replaceToken(n.replace(/_/g,"")),!0}return!1}};var Gt=class extends H{constructor(n,r){super(),this.tokens=n,this.nameManager=r}process(){return this.tokens.matches2(t._catch,t.braceL)?(this.tokens.copyToken(),this.tokens.appendCode(` (${this.nameManager.claimFreeName("e")})`),!0):!1}};var Xt=class extends H{constructor(n,r){super(),this.tokens=n,this.nameManager=r}process(){if(this.tokens.matches1(t.nullishCoalescing)){let o=this.tokens.currentToken();return this.tokens.tokens[o.nullishStartIndex].isAsyncOperation?this.tokens.replaceTokenTrimmingLeftWhitespace(", async () => ("):this.tokens.replaceTokenTrimmingLeftWhitespace(", () => ("),!0}if(this.tokens.matches1(t._delete)&&this.tokens.tokenAtRelativeIndex(1).isOptionalChainStart)return this.tokens.removeInitialToken(),!0;let r=this.tokens.currentToken().subscriptStartIndex;if(r!=null&&this.tokens.tokens[r].isOptionalChainStart&&this.tokens.tokenAtRelativeIndex(-1).type!==t._super){let o=this.nameManager.claimFreeName("_"),i;if(r>0&&this.tokens.matches1AtIndex(r-1,t._delete)&&this.isLastSubscriptInChain()?i=`${o} => delete ${o}`:i=`${o} => ${o}`,this.tokens.tokens[r].isAsyncOperation&&(i=`async ${i}`),this.tokens.matches2(t.questionDot,t.parenL)||this.tokens.matches2(t.questionDot,t.lessThan))this.justSkippedSuper()&&this.tokens.appendCode(".bind(this)"),this.tokens.replaceTokenTrimmingLeftWhitespace(`, 'optionalCall', ${i}`);else if(this.tokens.matches2(t.questionDot,t.bracketL))this.tokens.replaceTokenTrimmingLeftWhitespace(`, 'optionalAccess', ${i}`);else if(this.tokens.matches1(t.questionDot))this.tokens.replaceTokenTrimmingLeftWhitespace(`, 'optionalAccess', ${i}.`);else if(this.tokens.matches1(t.dot))this.tokens.replaceTokenTrimmingLeftWhitespace(`, 'access', ${i}.`);else if(this.tokens.matches1(t.bracketL))this.tokens.replaceTokenTrimmingLeftWhitespace(`, 'access', ${i}[`);else if(this.tokens.matches1(t.parenL))this.justSkippedSuper()&&this.tokens.appendCode(".bind(this)"),this.tokens.replaceTokenTrimmingLeftWhitespace(`, 'call', ${i}(`);else throw new Error("Unexpected subscript operator in optional chain.");return!0}return!1}isLastSubscriptInChain(){let n=0;for(let r=this.tokens.currentIndex()+1;;r++){if(r>=this.tokens.tokens.length)throw new Error("Reached the end of the code while finding the end of the access chain.");if(this.tokens.tokens[r].isOptionalChainStart?n++:this.tokens.tokens[r].isOptionalChainEnd&&n--,n<0)return!0;if(n===0&&this.tokens.tokens[r].subscriptStartIndex!=null)return!1}}justSkippedSuper(){let n=0,r=this.tokens.currentIndex()-1;for(;;){if(r<0)throw new Error("Reached the start of the code while finding the start of the access chain.");if(this.tokens.tokens[r].isOptionalChainStart?n--:this.tokens.tokens[r].isOptionalChainEnd&&n++,n<0)return!1;if(n===0&&this.tokens.tokens[r].subscriptStartIndex!=null)return this.tokens.tokens[r-1].type===t._super;r--}}};var Jt=class extends H{constructor(n,r,o,i){super(),this.rootTransformer=n,this.tokens=r,this.importProcessor=o,this.options=i}process(){let n=this.tokens.currentIndex();if(this.tokens.identifierName()==="createReactClass"){let r=this.importProcessor&&this.importProcessor.getIdentifierReplacement("createReactClass");return r?this.tokens.replaceToken(`(0, ${r})`):this.tokens.copyToken(),this.tryProcessCreateClassCall(n),!0}if(this.tokens.matches3(t.name,t.dot,t.name)&&this.tokens.identifierName()==="React"&&this.tokens.identifierNameAtIndex(this.tokens.currentIndex()+2)==="createClass"){let r=this.importProcessor&&this.importProcessor.getIdentifierReplacement("React")||"React";return r?(this.tokens.replaceToken(r),this.tokens.copyToken(),this.tokens.copyToken()):(this.tokens.copyToken(),this.tokens.copyToken(),this.tokens.copyToken()),this.tryProcessCreateClassCall(n),!0}return!1}tryProcessCreateClassCall(n){let r=this.findDisplayName(n);r&&this.classNeedsDisplayName()&&(this.tokens.copyExpectedToken(t.parenL),this.tokens.copyExpectedToken(t.braceL),this.tokens.appendCode(`displayName: '${r}',`),this.rootTransformer.processBalancedCode(),this.tokens.copyExpectedToken(t.braceR),this.tokens.copyExpectedToken(t.parenR))}findDisplayName(n){return n<2?null:this.tokens.matches2AtIndex(n-2,t.name,t.eq)?this.tokens.identifierNameAtIndex(n-2):n>=2&&this.tokens.tokens[n-2].identifierRole===b.ObjectKey?this.tokens.identifierNameAtIndex(n-2):this.tokens.matches2AtIndex(n-2,t._export,t._default)?this.getDisplayNameFromFilename():null}getDisplayNameFromFilename(){let r=(this.options.filePath||"unknown").split("/"),o=r[r.length-1],i=o.lastIndexOf("."),c=i===-1?o:o.slice(0,i);return c==="index"&&r[r.length-2]?r[r.length-2]:c}classNeedsDisplayName(){let n=this.tokens.currentIndex();if(!this.tokens.matches2(t.parenL,t.braceL))return!1;let r=n+1,o=this.tokens.tokens[r].contextId;if(o==null)throw new Error("Expected non-null context ID on object open-brace.");for(;n<this.tokens.tokens.length;n++){let i=this.tokens.tokens[n];if(i.type===t.braceR&&i.contextId===o){n++;break}if(this.tokens.identifierNameAtIndex(n)==="displayName"&&this.tokens.tokens[n].identifierRole===b.ObjectKey&&i.contextId===o)return!1}if(n===this.tokens.tokens.length)throw new Error("Unexpected end of input when processing React class.");return this.tokens.matches1AtIndex(n,t.parenR)||this.tokens.matches2AtIndex(n,t.comma,t.parenR)}};var Kt=class e extends H{__init(){this.extractedDefaultExportName=null}constructor(n,r){super(),this.tokens=n,this.filePath=r,e.prototype.__init.call(this)}setExtractedDefaultExportName(n){this.extractedDefaultExportName=n}getPrefixCode(){return`
      (function () {
        var enterModule = require('react-hot-loader').enterModule;
        enterModule && enterModule(module);
      })();`.replace(/\s+/g," ").trim()}getSuffixCode(){let n=new Set;for(let o of this.tokens.tokens)!o.isType&&rn(o)&&o.identifierRole!==b.ImportDeclaration&&n.add(this.tokens.identifierNameForToken(o));let r=Array.from(n).map(o=>({variableName:o,uniqueLocalName:o}));return this.extractedDefaultExportName&&r.push({variableName:this.extractedDefaultExportName,uniqueLocalName:"default"}),`
;(function () {
  var reactHotLoader = require('react-hot-loader').default;
  var leaveModule = require('react-hot-loader').leaveModule;
  if (!reactHotLoader) {
    return;
  }
${r.map(({variableName:o,uniqueLocalName:i})=>`  reactHotLoader.register(${o}, "${i}", ${JSON.stringify(this.filePath||"")});`).join(`
`)}
  leaveModule(module);
})();`}process(){return!1}};var Ku=new Set(["break","case","catch","class","const","continue","debugger","default","delete","do","else","export","extends","finally","for","function","if","import","in","instanceof","new","return","super","switch","this","throw","try","typeof","var","void","while","with","yield","enum","implements","interface","let","package","private","protected","public","static","await","false","null","true"]);function Fn(e){if(e.length===0||!ye[e.charCodeAt(0)])return!1;for(let n=1;n<e.length;n++)if(!Y[e.charCodeAt(n)])return!1;return!Ku.has(e)}var Qt=class extends H{constructor(n,r,o){super(),this.rootTransformer=n,this.tokens=r,this.isImportsTransformEnabled=o}process(){return this.rootTransformer.processPossibleArrowParamEnd()||this.rootTransformer.processPossibleAsyncArrowWithTypeParams()||this.rootTransformer.processPossibleTypeRange()?!0:this.tokens.matches1(t._public)||this.tokens.matches1(t._protected)||this.tokens.matches1(t._private)||this.tokens.matches1(t._abstract)||this.tokens.matches1(t._readonly)||this.tokens.matches1(t._override)||this.tokens.matches1(t.nonNullAssertion)?(this.tokens.removeInitialToken(),!0):this.tokens.matches1(t._enum)||this.tokens.matches2(t._const,t._enum)?(this.processEnum(),!0):this.tokens.matches2(t._export,t._enum)||this.tokens.matches3(t._export,t._const,t._enum)?(this.processEnum(!0),!0):!1}processEnum(n=!1){for(this.tokens.removeInitialToken();this.tokens.matches1(t._const)||this.tokens.matches1(t._enum);)this.tokens.removeToken();let r=this.tokens.identifierName();this.tokens.removeToken(),n&&!this.isImportsTransformEnabled&&this.tokens.appendCode("export "),this.tokens.appendCode(`var ${r}; (function (${r})`),this.tokens.copyExpectedToken(t.braceL),this.processEnumBody(r),this.tokens.copyExpectedToken(t.braceR),n&&this.isImportsTransformEnabled?this.tokens.appendCode(`)(${r} || (exports.${r} = ${r} = {}));`):this.tokens.appendCode(`)(${r} || (${r} = {}));`)}processEnumBody(n){let r=null;for(;!this.tokens.matches1(t.braceR);){let{nameStringCode:o,variableName:i}=this.extractEnumKeyInfo(this.tokens.currentToken());this.tokens.removeInitialToken(),this.tokens.matches3(t.eq,t.string,t.comma)||this.tokens.matches3(t.eq,t.string,t.braceR)?this.processStringLiteralEnumMember(n,o,i):this.tokens.matches1(t.eq)?this.processExplicitValueEnumMember(n,o,i):this.processImplicitValueEnumMember(n,o,i,r),this.tokens.matches1(t.comma)&&this.tokens.removeToken(),i!=null?r=i:r=`${n}[${o}]`}}extractEnumKeyInfo(n){if(n.type===t.name){let r=this.tokens.identifierNameForToken(n);return{nameStringCode:`"${r}"`,variableName:Fn(r)?r:null}}else if(n.type===t.string){let r=this.tokens.stringValueForToken(n);return{nameStringCode:this.tokens.code.slice(n.start,n.end),variableName:Fn(r)?r:null}}else throw new Error("Expected name or string at beginning of enum element.")}processStringLiteralEnumMember(n,r,o){o!=null?(this.tokens.appendCode(`const ${o}`),this.tokens.copyToken(),this.tokens.copyToken(),this.tokens.appendCode(`; ${n}[${r}] = ${o};`)):(this.tokens.appendCode(`${n}[${r}]`),this.tokens.copyToken(),this.tokens.copyToken(),this.tokens.appendCode(";"))}processExplicitValueEnumMember(n,r,o){let i=this.tokens.currentToken().rhsEndIndex;if(i==null)throw new Error("Expected rhsEndIndex on enum assign.");if(o!=null){for(this.tokens.appendCode(`const ${o}`),this.tokens.copyToken();this.tokens.currentIndex()<i;)this.rootTransformer.processToken();this.tokens.appendCode(`; ${n}[${n}[${r}] = ${o}] = ${r};`)}else{for(this.tokens.appendCode(`${n}[${n}[${r}]`),this.tokens.copyToken();this.tokens.currentIndex()<i;)this.rootTransformer.processToken();this.tokens.appendCode(`] = ${r};`)}}processImplicitValueEnumMember(n,r,o,i){let c=i!=null?`${i} + 1`:"0";o!=null&&(this.tokens.appendCode(`const ${o} = ${c}; `),c=o),this.tokens.appendCode(`${n}[${n}[${r}] = ${c}] = ${r};`)}};var Yt=class e{__init(){this.transformers=[]}__init2(){this.generatedVariables=[]}constructor(n,r,o,i){e.prototype.__init.call(this),e.prototype.__init2.call(this),this.nameManager=n.nameManager,this.helperManager=n.helperManager;let{tokenProcessor:c,importProcessor:p}=n;this.tokens=c,this.isImportsTransformEnabled=r.includes("imports"),this.isReactHotLoaderTransformEnabled=r.includes("react-hot-loader"),this.disableESTransforms=!!i.disableESTransforms,i.disableESTransforms||(this.transformers.push(new Xt(c,this.nameManager)),this.transformers.push(new zt(c)),this.transformers.push(new Gt(c,this.nameManager))),r.includes("jsx")&&(i.jsxRuntime!=="preserve"&&this.transformers.push(new yt(this,c,p,this.nameManager,i)),this.transformers.push(new Jt(this,c,p,i)));let h=null;if(r.includes("react-hot-loader")){if(!i.filePath)throw new Error("filePath is required when using the react-hot-loader transform.");h=new Kt(c,i.filePath),this.transformers.push(h)}if(r.includes("imports")){if(p===null)throw new Error("Expected non-null importProcessor with imports transform enabled.");this.transformers.push(new Ut(this,c,p,this.nameManager,this.helperManager,h,o,!!i.enableLegacyTypeScriptModuleInterop,r.includes("typescript"),r.includes("flow"),!!i.preserveDynamicImport,!!i.keepUnusedImports))}else this.transformers.push(new Ht(c,this.nameManager,this.helperManager,h,r.includes("typescript"),r.includes("flow"),!!i.keepUnusedImports,i));r.includes("flow")&&this.transformers.push(new Vt(this,c,r.includes("imports"))),r.includes("typescript")&&this.transformers.push(new Qt(this,c,r.includes("imports"))),r.includes("jest")&&this.transformers.push(new Wt(this,c,this.nameManager,p))}transform(){this.tokens.reset(),this.processBalancedCode();let r=this.isImportsTransformEnabled?'"use strict";':"";for(let p of this.transformers)r+=p.getPrefixCode();r+=this.helperManager.emitHelpers(),r+=this.generatedVariables.map(p=>` var ${p};`).join("");for(let p of this.transformers)r+=p.getHoistedCode();let o="";for(let p of this.transformers)o+=p.getSuffixCode();let i=this.tokens.finish(),{code:c}=i;if(c.startsWith("#!")){let p=c.indexOf(`
`);return p===-1&&(p=c.length,c+=`
`),{code:c.slice(0,p+1)+r+c.slice(p+1)+o,mappings:this.shiftMappings(i.mappings,r.length)}}else return{code:r+c+o,mappings:this.shiftMappings(i.mappings,r.length)}}processBalancedCode(){let n=0,r=0;for(;!this.tokens.isAtEnd();){if(this.tokens.matches1(t.braceL)||this.tokens.matches1(t.dollarBraceL))n++;else if(this.tokens.matches1(t.braceR)){if(n===0)return;n--}if(this.tokens.matches1(t.parenL))r++;else if(this.tokens.matches1(t.parenR)){if(r===0)return;r--}this.processToken()}}processToken(){if(this.tokens.matches1(t._class)){this.processClass();return}for(let n of this.transformers)if(n.process())return;this.tokens.copyToken()}processNamedClass(){if(!this.tokens.matches2(t._class,t.name))throw new Error("Expected identifier for exported class name.");let n=this.tokens.identifierNameAtIndex(this.tokens.currentIndex()+1);return this.processClass(),n}processClass(){let n=co(this,this.tokens,this.nameManager,this.disableESTransforms),r=(n.headerInfo.isExpression||!n.headerInfo.className)&&n.staticInitializerNames.length+n.instanceInitializerNames.length>0,o=n.headerInfo.className;r&&(o=this.nameManager.claimFreeName("_class"),this.generatedVariables.push(o),this.tokens.appendCode(` (${o} =`));let c=this.tokens.currentToken().contextId;if(c==null)throw new Error("Expected class to have a context ID.");for(this.tokens.copyExpectedToken(t._class);!this.tokens.matchesContextIdAndLabel(t.braceL,c);)this.processToken();this.processClassBody(n,o);let p=n.staticInitializerNames.map(h=>`${o}.${h}()`);r?this.tokens.appendCode(`, ${p.map(h=>`${h}, `).join("")}${o})`):n.staticInitializerNames.length>0&&this.tokens.appendCode(` ${p.map(h=>`${h};`).join(" ")}`)}processClassBody(n,r){let{headerInfo:o,constructorInsertPos:i,constructorInitializerStatements:c,fields:p,instanceInitializerNames:h,rangesToRemove:x}=n,k=0,y=0,w=this.tokens.currentToken().contextId;if(w==null)throw new Error("Expected non-null context ID on class.");this.tokens.copyExpectedToken(t.braceL),this.isReactHotLoaderTransformEnabled&&this.tokens.appendCode("__reactstandin__regenerateByEval(key, code) {this[key] = eval(code);}");let I=c.length+h.length>0;if(i===null&&I){let A=this.makeConstructorInitCode(c,h,r);if(o.hasSuperclass){let O=this.nameManager.claimFreeName("args");this.tokens.appendCode(`constructor(...${O}) { super(...${O}); ${A}; }`)}else this.tokens.appendCode(`constructor() { ${A}; }`)}for(;!this.tokens.matchesContextIdAndLabel(t.braceR,w);)if(k<p.length&&this.tokens.currentIndex()===p[k].start){let A=!1;for(this.tokens.matches1(t.bracketL)?this.tokens.copyTokenWithPrefix(`${p[k].initializerName}() {this`):this.tokens.matches1(t.string)||this.tokens.matches1(t.num)?(this.tokens.copyTokenWithPrefix(`${p[k].initializerName}() {this[`),A=!0):this.tokens.copyTokenWithPrefix(`${p[k].initializerName}() {this.`);this.tokens.currentIndex()<p[k].end;)A&&this.tokens.currentIndex()===p[k].equalsIndex&&this.tokens.appendCode("]"),this.processToken();this.tokens.appendCode("}"),k++}else if(y<x.length&&this.tokens.currentIndex()>=x[y].start){for(this.tokens.currentIndex()<x[y].end&&this.tokens.removeInitialToken();this.tokens.currentIndex()<x[y].end;)this.tokens.removeToken();y++}else this.tokens.currentIndex()===i?(this.tokens.copyToken(),I&&this.tokens.appendCode(`;${this.makeConstructorInitCode(c,h,r)};`),this.processToken()):this.processToken();this.tokens.copyExpectedToken(t.braceR)}makeConstructorInitCode(n,r,o){return[...n,...r.map(i=>`${o}.prototype.${i}.call(this)`)].join(";")}processPossibleArrowParamEnd(){if(this.tokens.matches2(t.parenR,t.colon)&&this.tokens.tokenAtRelativeIndex(1).isType){let n=this.tokens.currentIndex()+1;for(;this.tokens.tokens[n].isType;)n++;if(this.tokens.matches1AtIndex(n,t.arrow)){for(this.tokens.removeInitialToken();this.tokens.currentIndex()<n;)this.tokens.removeToken();return this.tokens.replaceTokenTrimmingLeftWhitespace(") =>"),!0}}return!1}processPossibleAsyncArrowWithTypeParams(){if(!this.tokens.matchesContextual(l._async)&&!this.tokens.matches1(t._async))return!1;let n=this.tokens.tokenAtRelativeIndex(1);if(n.type!==t.lessThan||!n.isType)return!1;let r=this.tokens.currentIndex()+1;for(;this.tokens.tokens[r].isType;)r++;if(this.tokens.matches1AtIndex(r,t.parenL)){for(this.tokens.replaceToken("async ("),this.tokens.removeInitialToken();this.tokens.currentIndex()<r;)this.tokens.removeToken();return this.tokens.removeToken(),this.processBalancedCode(),this.processToken(),!0}return!1}processPossibleTypeRange(){if(this.tokens.currentToken().isType){for(this.tokens.removeInitialToken();this.tokens.currentToken().isType;)this.tokens.removeToken();return!0}return!1}shiftMappings(n,r){for(let o=0;o<n.length;o++){let i=n[o];i!==void 0&&(n[o]=i+r)}return n}};var Qu=Xr(j1());function lo(e){let n=new Set;for(let r=0;r<e.tokens.length;r++)e.matches1AtIndex(r,t._import)&&!e.matches3AtIndex(r,t._import,t.name,t.eq)&&Yu(e,r,n);return n}function Yu(e,n,r){n++,!e.matches1AtIndex(n,t.parenL)&&(e.matches1AtIndex(n,t.name)&&(r.add(e.identifierNameAtIndex(n)),n++,e.matches1AtIndex(n,t.comma)&&n++),e.matches1AtIndex(n,t.star)&&(n+=2,r.add(e.identifierNameAtIndex(n)),n++),e.matches1AtIndex(n,t.braceL)&&(n++,Zu(e,n,r)))}function Zu(e,n,r){for(;;){if(e.matches1AtIndex(n,t.braceR))return;let o=me(e,n);if(n=o.endIndex,o.isType||r.add(o.rightName),e.matches2AtIndex(n,t.comma,t.braceR))return;if(e.matches1AtIndex(n,t.braceR))return;if(e.matches1AtIndex(n,t.comma))n++;else throw new Error(`Unexpected token: ${JSON.stringify(e.tokens[n])}`)}}function F1(e,n){ti(n);try{let r=ep(e,n),i=new Yt(r,n.transforms,!!n.enableLegacyBabel5ModuleInterop,n).transform(),c={code:i.code};if(n.sourceMapOptions){if(!n.filePath)throw new Error("filePath must be specified when generating a source map.");c={...c,sourceMap:ms(i,n.filePath,n.sourceMapOptions,e,r.tokenProcessor.tokens)}}return c}catch(r){throw n.filePath&&(r.message=`Error transforming ${n.filePath}: ${r.message}`),r}}function ep(e,n){let r=n.transforms.includes("jsx"),o=n.transforms.includes("typescript"),i=n.transforms.includes("flow"),c=n.disableESTransforms===!0,p=R1(e,r,o,i),h=p.tokens,x=p.scopes,k=new It(e,h),y=new un(k),w=new Ft(e,h,i,c,y),I=!!n.enableLegacyTypeScriptModuleInterop,A=null;return n.transforms.includes("imports")?(A=new _t(k,w,I,n,n.transforms.includes("typescript"),!!n.keepUnusedImports,y),A.preprocessTokens(),pn(w,x,A.getGlobalNames()),n.transforms.includes("typescript")&&!n.keepUnusedImports&&A.pruneTypeOnlyImports()):n.transforms.includes("typescript")&&!n.keepUnusedImports&&pn(w,x,lo(w)),{tokenProcessor:w,scopes:x,nameManager:k,importProcessor:A,helperManager:y}}var M1={metadata:"music.metadata",streaming:"music.stream",lyrics:"lyrics"},tp=["Queue","Playback","Favorites","Playlists","Dashboard","Discovery","Events","Shell","Navigation","Ytdlp","Metadata","Streaming","Lyrics"],np=["index.ts","index.tsx","index.js","src/index.ts","src/index.tsx","src/index.js","dist/index.js"],rp=e=>/\.(tsx?|jsx?|mjs|cjs|json)$/.test(e)&&!e.endsWith(".d.ts")&&!/\.(test|spec)\.[cm]?[jt]sx?$/.test(e)&&!/(^|\/)(test|tests|__tests__|__mocks__|node_modules)\//.test(e)&&!/(^|\/)(package-lock|tsconfig[^/]*|package)\.json$/.test(e),sp=e=>e.toLowerCase().replace(/^@/,"").replace(/[^a-z0-9]+/g,"-").replace(/^-+|-+$/g,"").slice(0,80)||"plugin",op=/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$/,ip=new Set(["localhost","example.com","example.org","www.w3.org"]),ap=e=>{let n=new Set;for(let r of e)for(let[,o]of r.matchAll(/https?:\/\/([a-zA-Z0-9-]+(?:\.[a-zA-Z0-9-]+)+)/g)){let i=(o??"").toLowerCase();ip.has(i)||n.add(i)}return[...n].sort()},cp=(e,n)=>{if(e.endsWith(".json"))return`module.exports = ${n.trim()||"null"};`;let r=/\.[jt]sx$/.test(e);return F1(n,{transforms:[.../\.tsx?$/.test(e)?["typescript"]:[],...r?["jsx"]:[],"imports"],jsxRuntime:"automatic",production:!0,filePath:e}).code},B1=(e,n)=>{let r=Object.keys(e).filter(E=>E==="package.json"||E.endsWith("/package.json")).filter(E=>!E.includes("node_modules/")).sort((E,$)=>E.split("/").length-$.split("/").length)[0];if(!r)throw new Error("This is not a Nuclear plugin: there is no package.json");let o=r.slice(0,-12),i=JSON.parse(e[r]);if(!i.name)throw new Error("package.json has no name");let c={};for(let[E,$]of Object.entries(e)){if(!E.startsWith(o))continue;let Q=E.slice(o.length);rp(Q)&&(c[Q]=$)}let p=[i.main?.replace(/^\.\//,""),...np].find(E=>!!E&&E in c);if(!p)throw new Error(`Can't find the plugin's entry file (${i.main??"index.ts"})`);let h=i.nuclear?.categories??(i.nuclear?.category?[i.nuclear.category]:[]),x=Object.values(c),k={supported:[...new Set(h.flatMap(E=>M1[E]??[]))],unsupportedCategories:h.filter(E=>!(E in M1)),stubbedApis:tp.filter(E=>x.some($=>new RegExp(`\\.${E}\\s*\\.`).test($))),uiImports:[...new Set(x.flatMap(E=>[...E.matchAll(/from\s+['"](react(?:\/[\w-]+)?|@nuclearplayer\/ui)['"]/g)].map(([,$])=>$)))].sort(),hosts:ap(x),modules:Object.keys(c).length};if(k.supported.length===0){let E=h.length>0?h.join(", "):"no provider categories";throw new Error(`Raygun can't run this Nuclear plugin yet: it provides ${E}`)}let y=Object.entries(c).map(([E,$])=>`${JSON.stringify(E)}: function (exports, require, module) {
${cp(E,$)}
}`).join(`,
`),w={id:i.name,name:i.nuclear?.displayName??i.name,version:i.version??"0.0.0",categories:h},I=[`// Nuclear plugin ${i.name} ${i.version??""}, converted for Raygun by @raygun/nuclear-compat.`,n,"(function () {",`var definitions = {
${y}
};`,`__nuclearShim.run({ entry: ${JSON.stringify(p)}, definitions: definitions, plugin: ${JSON.stringify(w)} });`,"})();",""].join(`
`),A=typeof i.author=="string"?i.author:i.author?.name;return{manifest:{id:`nuclear.${sp(i.name)}`,name:(i.nuclear?.displayName??i.name).slice(0,80),version:i.version&&op.test(i.version)?i.version:"0.0.0",apiVersion:1,runtime:"js",entry:"dist/index.js",description:i.description??null,author:A??null,provides:k.supported,permissions:{net:k.hosts},contentRating:"safe",nuclear:{package:i.name,report:k}},bundle:I,report:k}};globalThis.__convert=e=>{try{return JSON.stringify(B1(JSON.parse(e),po))}catch(n){return JSON.stringify({error:n instanceof Error?n.message:String(n)})}};})();
