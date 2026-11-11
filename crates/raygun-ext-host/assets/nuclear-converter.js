// Raygun Nuclear plugin converter. Built by packages/nuclear-compat (pnpm build); do not edit.
"use strict";(()=>{var na=Object.create;var uo=Object.defineProperty;var ra=Object.getOwnPropertyDescriptor;var sa=Object.getOwnPropertyNames;var oa=Object.getPrototypeOf,ia=Object.prototype.hasOwnProperty;var en=(e,n)=>()=>(n||e((n={exports:{}}).exports,n),n.exports);var aa=(e,n,r,o)=>{if(n&&typeof n=="object"||typeof n=="function")for(let i of sa(n))!ia.call(e,i)&&i!==r&&uo(e,i,{get:()=>n[i],enumerable:!(o=ra(n,i))||o.enumerable});return e};var Jr=(e,n,r)=>(r=e!=null?na(oa(e)):{},aa(n||!e||!e.__esModule?uo(r,"default",{value:e,enumerable:!0}):r,e));var fn=en(ke=>{"use strict";var Ya=ke&&ke.__extends||(function(){var e=function(n,r){return e=Object.setPrototypeOf||{__proto__:[]}instanceof Array&&function(o,i){o.__proto__=i}||function(o,i){for(var c in i)i.hasOwnProperty(c)&&(o[c]=i[c])},e(n,r)};return function(n,r){e(n,r);function o(){this.constructor=n}n.prototype=r===null?Object.create(r):(o.prototype=r.prototype,new o)}})();Object.defineProperty(ke,"__esModule",{value:!0});ke.DetailContext=ke.NoopContext=ke.VError=void 0;var jo=(function(e){Ya(n,e);function n(r,o){var i=e.call(this,o)||this;return i.path=r,Object.setPrototypeOf(i,n.prototype),i}return n})(Error);ke.VError=jo;var Qa=(function(){function e(){}return e.prototype.fail=function(n,r,o){return!1},e.prototype.unionResolver=function(){return this},e.prototype.createContext=function(){return this},e.prototype.resolveUnion=function(n){},e})();ke.NoopContext=Qa;var Fo=(function(){function e(){this._propNames=[""],this._messages=[null],this._score=0}return e.prototype.fail=function(n,r,o){return this._propNames.push(n),this._messages.push(r),this._score+=o,!1},e.prototype.unionResolver=function(){return new Za},e.prototype.resolveUnion=function(n){for(var r,o,i=n,c=null,p=0,h=i.contexts;p<h.length;p++){var g=h[p];(!c||g._score>=c._score)&&(c=g)}c&&c._score>0&&((r=this._propNames).push.apply(r,c._propNames),(o=this._messages).push.apply(o,c._messages))},e.prototype.getError=function(n){for(var r=[],o=this._propNames.length-1;o>=0;o--){var i=this._propNames[o];n+=typeof i=="number"?"["+i+"]":i?"."+i:"";var c=this._messages[o];c&&r.push(n+" "+c)}return new jo(n,r.join("; "))},e.prototype.getErrorDetail=function(n){for(var r=[],o=this._propNames.length-1;o>=0;o--){var i=this._propNames[o];n+=typeof i=="number"?"["+i+"]":i?"."+i:"";var c=this._messages[o];c&&r.push({path:n,message:c})}for(var p=null,o=r.length-1;o>=0;o--)p&&(r[o].nested=[p]),p=r[o];return p},e})();ke.DetailContext=Fo;var Za=(function(){function e(){this.contexts=[]}return e.prototype.createContext=function(){var n=new Fo;return this.contexts.push(n),n},e})()});var vs=en(w=>{"use strict";var oe=w&&w.__extends||(function(){var e=function(n,r){return e=Object.setPrototypeOf||{__proto__:[]}instanceof Array&&function(o,i){o.__proto__=i}||function(o,i){for(var c in i)i.hasOwnProperty(c)&&(o[c]=i[c])},e(n,r)};return function(n,r){e(n,r);function o(){this.constructor=n}n.prototype=r===null?Object.create(r):(o.prototype=r.prototype,new o)}})();Object.defineProperty(w,"__esModule",{value:!0});w.basicTypes=w.BasicType=w.TParamList=w.TParam=w.param=w.TFunc=w.func=w.TProp=w.TOptional=w.opt=w.TIface=w.iface=w.TEnumLiteral=w.enumlit=w.TEnumType=w.enumtype=w.TIntersection=w.intersection=w.TUnion=w.union=w.TTuple=w.tuple=w.TArray=w.array=w.TLiteral=w.lit=w.TName=w.name=w.TType=void 0;var $o=fn(),ne=(function(){function e(){}return e})();w.TType=ne;function Ee(e){return typeof e=="string"?Uo(e):e}function ys(e,n){var r=e[n];if(!r)throw new Error("Unknown type "+n);return r}function Uo(e){return new xs(e)}w.name=Uo;var xs=(function(e){oe(n,e);function n(r){var o=e.call(this)||this;return o.name=r,o._failMsg="is not a "+r,o}return n.prototype.getChecker=function(r,o,i){var c=this,p=ys(r,this.name),h=p.getChecker(r,o,i);return p instanceof Q||p instanceof n?h:function(g,y){return h(g,y)?!0:y.fail(null,c._failMsg,0)}},n})(ne);w.TName=xs;function ec(e){return new bs(e)}w.lit=ec;var bs=(function(e){oe(n,e);function n(r){var o=e.call(this)||this;return o.value=r,o.name=JSON.stringify(r),o._failMsg="is not "+o.name,o}return n.prototype.getChecker=function(r,o){var i=this;return function(c,p){return c===i.value?!0:p.fail(null,i._failMsg,-1)}},n})(ne);w.TLiteral=bs;function tc(e){return new Ho(Ee(e))}w.array=tc;var Ho=(function(e){oe(n,e);function n(r){var o=e.call(this)||this;return o.ttype=r,o}return n.prototype.getChecker=function(r,o){var i=this.ttype.getChecker(r,o);return function(c,p){if(!Array.isArray(c))return p.fail(null,"is not an array",0);for(var h=0;h<c.length;h++){var g=i(c[h],p);if(!g)return p.fail(h,null,1)}return!0}},n})(ne);w.TArray=Ho;function nc(){for(var e=[],n=0;n<arguments.length;n++)e[n]=arguments[n];return new Vo(e.map(function(r){return Ee(r)}))}w.tuple=nc;var Vo=(function(e){oe(n,e);function n(r){var o=e.call(this)||this;return o.ttypes=r,o}return n.prototype.getChecker=function(r,o){var i=this.ttypes.map(function(p){return p.getChecker(r,o)}),c=function(p,h){if(!Array.isArray(p))return h.fail(null,"is not an array",0);for(var g=0;g<i.length;g++){var y=i[g](p[g],h);if(!y)return h.fail(g,null,1)}return!0};return o?function(p,h){return c(p,h)?p.length<=i.length?!0:h.fail(i.length,"is extraneous",2):!1}:c},n})(ne);w.TTuple=Vo;function rc(){for(var e=[],n=0;n<arguments.length;n++)e[n]=arguments[n];return new Wo(e.map(function(r){return Ee(r)}))}w.union=rc;var Wo=(function(e){oe(n,e);function n(r){var o=e.call(this)||this;o.ttypes=r;var i=r.map(function(p){return p instanceof xs||p instanceof bs?p.name:null}).filter(function(p){return p}),c=r.length-i.length;return i.length?(c>0&&i.push(c+" more"),o._failMsg="is none of "+i.join(", ")):o._failMsg="is none of "+c+" types",o}return n.prototype.getChecker=function(r,o){var i=this,c=this.ttypes.map(function(p){return p.getChecker(r,o)});return function(p,h){for(var g=h.unionResolver(),y=0;y<c.length;y++){var x=c[y](p,g.createContext());if(x)return!0}return h.resolveUnion(g),h.fail(null,i._failMsg,0)}},n})(ne);w.TUnion=Wo;function sc(){for(var e=[],n=0;n<arguments.length;n++)e[n]=arguments[n];return new zo(e.map(function(r){return Ee(r)}))}w.intersection=sc;var zo=(function(e){oe(n,e);function n(r){var o=e.call(this)||this;return o.ttypes=r,o}return n.prototype.getChecker=function(r,o){var i=new Set,c=this.ttypes.map(function(p){return p.getChecker(r,o,i)});return function(p,h){var g=c.every(function(y){return y(p,h)});return g?!0:h.fail(null,null,0)}},n})(ne);w.TIntersection=zo;function oc(e){return new _s(e)}w.enumtype=oc;var _s=(function(e){oe(n,e);function n(r){var o=e.call(this)||this;return o.members=r,o.validValues=new Set,o._failMsg="is not a valid enum value",o.validValues=new Set(Object.keys(r).map(function(i){return r[i]})),o}return n.prototype.getChecker=function(r,o){var i=this;return function(c,p){return i.validValues.has(c)?!0:p.fail(null,i._failMsg,0)}},n})(ne);w.TEnumType=_s;function ic(e,n){return new Xo(e,n)}w.enumlit=ic;var Xo=(function(e){oe(n,e);function n(r,o){var i=e.call(this)||this;return i.enumName=r,i.prop=o,i._failMsg="is not "+r+"."+o,i}return n.prototype.getChecker=function(r,o){var i=this,c=ys(r,this.enumName);if(!(c instanceof _s))throw new Error("Type "+this.enumName+" used in enumlit is not an enum type");var p=c.members[this.prop];if(!c.members.hasOwnProperty(this.prop))throw new Error("Unknown value "+this.enumName+"."+this.prop+" used in enumlit");return function(h,g){return h===p?!0:g.fail(null,i._failMsg,-1)}},n})(ne);w.TEnumLiteral=Xo;function ac(e){return Object.keys(e).map(function(n){return cc(n,e[n])})}function cc(e,n){return n instanceof ws?new ks(e,n.ttype,!0):new ks(e,Ee(n),!1)}function lc(e,n){return new Jo(e,ac(n))}w.iface=lc;var Jo=(function(e){oe(n,e);function n(r,o){var i=e.call(this)||this;return i.bases=r,i.props=o,i.propSet=new Set(o.map(function(c){return c.name})),i}return n.prototype.getChecker=function(r,o,i){var c=this,p=this.bases.map(function(T){return ys(r,T).getChecker(r,o)}),h=this.props.map(function(T){return T.ttype.getChecker(r,o)}),g=new $o.NoopContext,y=this.props.map(function(T,E){return!T.isOpt&&!h[E](void 0,g)}),x=function(T,E){if(typeof T!="object"||T===null)return E.fail(null,"is not an object",0);for(var O=0;O<p.length;O++)if(!p[O](T,E))return!1;for(var O=0;O<h.length;O++){var q=c.props[O].name,G=T[q];if(G===void 0){if(y[O])return E.fail(q,"is missing",1)}else{var N=h[O](G,E);if(!N)return E.fail(q,null,1)}}return!0};if(!o)return x;var v=this.propSet;return i&&(this.propSet.forEach(function(T){return i.add(T)}),v=i),function(T,E){if(!x(T,E))return!1;for(var O in T)if(!v.has(O))return E.fail(O,"is extraneous",2);return!0}},n})(ne);w.TIface=Jo;function uc(e){return new ws(Ee(e))}w.opt=uc;var ws=(function(e){oe(n,e);function n(r){var o=e.call(this)||this;return o.ttype=r,o}return n.prototype.getChecker=function(r,o){var i=this.ttype.getChecker(r,o);return function(c,p){return c===void 0||i(c,p)}},n})(ne);w.TOptional=ws;var ks=(function(){function e(n,r,o){this.name=n,this.ttype=r,this.isOpt=o}return e})();w.TProp=ks;function pc(e){for(var n=[],r=1;r<arguments.length;r++)n[r-1]=arguments[r];return new Go(new Yo(n),Ee(e))}w.func=pc;var Go=(function(e){oe(n,e);function n(r,o){var i=e.call(this)||this;return i.paramList=r,i.result=o,i}return n.prototype.getChecker=function(r,o){return function(i,c){return typeof i=="function"?!0:c.fail(null,"is not a function",0)}},n})(ne);w.TFunc=Go;function fc(e,n,r){return new Ko(e,Ee(n),!!r)}w.param=fc;var Ko=(function(){function e(n,r,o){this.name=n,this.ttype=r,this.isOpt=o}return e})();w.TParam=Ko;var Yo=(function(e){oe(n,e);function n(r){var o=e.call(this)||this;return o.params=r,o}return n.prototype.getChecker=function(r,o){var i=this,c=this.params.map(function(y){return y.ttype.getChecker(r,o)}),p=new $o.NoopContext,h=this.params.map(function(y,x){return!y.isOpt&&!c[x](void 0,p)}),g=function(y,x){if(!Array.isArray(y))return x.fail(null,"is not an array",0);for(var v=0;v<c.length;v++){var T=i.params[v];if(y[v]===void 0){if(h[v])return x.fail(T.name,"is missing",1)}else{var E=c[v](y[v],x);if(!E)return x.fail(T.name,null,1)}}return!0};return o?function(y,x){return g(y,x)?y.length<=c.length?!0:x.fail(c.length,"is extraneous",2):!1}:g},n})(ne);w.TParamList=Yo;var Q=(function(e){oe(n,e);function n(r,o){var i=e.call(this)||this;return i.validator=r,i.message=o,i}return n.prototype.getChecker=function(r,o){var i=this;return function(c,p){return i.validator(c)?!0:p.fail(null,i.message,0)}},n})(ne);w.BasicType=Q;w.basicTypes={any:new Q(function(e){return!0},"is invalid"),number:new Q(function(e){return typeof e=="number"},"is not a number"),object:new Q(function(e){return typeof e=="object"&&e},"is not an object"),boolean:new Q(function(e){return typeof e=="boolean"},"is not a boolean"),string:new Q(function(e){return typeof e=="string"},"is not a string"),symbol:new Q(function(e){return typeof e=="symbol"},"is not a symbol"),void:new Q(function(e){return e==null},"is not void"),undefined:new Q(function(e){return e===void 0},"is not undefined"),null:new Q(function(e){return e===null},"is not null"),never:new Q(function(e){return!1},"is unexpected"),Date:new Q(Bo("[object Date]"),"is not a Date"),RegExp:new Q(Bo("[object RegExp]"),"is not a RegExp")};var hc=Object.prototype.toString;function Bo(e){return function(n){return typeof n=="object"&&n&&hc.call(n)===e}}typeof Buffer<"u"&&(w.basicTypes.Buffer=new Q(function(e){return Buffer.isBuffer(e)},"is not a Buffer"));var dc=function(e){w.basicTypes[e.name]=new Q(function(n){return n instanceof e},"is not a "+e.name)};for(hn=0,gs=[Int8Array,Uint8Array,Uint8ClampedArray,Int16Array,Uint16Array,Int32Array,Uint32Array,Float32Array,Float64Array,ArrayBuffer];hn<gs.length;hn++)qo=gs[hn],dc(qo);var qo,hn,gs});var Ts=en(D=>{"use strict";var mc=D&&D.__spreadArrays||function(){for(var e=0,n=0,r=arguments.length;n<r;n++)e+=arguments[n].length;for(var o=Array(e),i=0,n=0;n<r;n++)for(var c=arguments[n],p=0,h=c.length;p<h;p++,i++)o[i]=c[p];return o};Object.defineProperty(D,"__esModule",{value:!0});D.Checker=D.createCheckers=void 0;var Tt=vs(),ot=fn(),$=vs();Object.defineProperty(D,"TArray",{enumerable:!0,get:function(){return $.TArray}});Object.defineProperty(D,"TEnumType",{enumerable:!0,get:function(){return $.TEnumType}});Object.defineProperty(D,"TEnumLiteral",{enumerable:!0,get:function(){return $.TEnumLiteral}});Object.defineProperty(D,"TFunc",{enumerable:!0,get:function(){return $.TFunc}});Object.defineProperty(D,"TIface",{enumerable:!0,get:function(){return $.TIface}});Object.defineProperty(D,"TLiteral",{enumerable:!0,get:function(){return $.TLiteral}});Object.defineProperty(D,"TName",{enumerable:!0,get:function(){return $.TName}});Object.defineProperty(D,"TOptional",{enumerable:!0,get:function(){return $.TOptional}});Object.defineProperty(D,"TParam",{enumerable:!0,get:function(){return $.TParam}});Object.defineProperty(D,"TParamList",{enumerable:!0,get:function(){return $.TParamList}});Object.defineProperty(D,"TProp",{enumerable:!0,get:function(){return $.TProp}});Object.defineProperty(D,"TTuple",{enumerable:!0,get:function(){return $.TTuple}});Object.defineProperty(D,"TType",{enumerable:!0,get:function(){return $.TType}});Object.defineProperty(D,"TUnion",{enumerable:!0,get:function(){return $.TUnion}});Object.defineProperty(D,"TIntersection",{enumerable:!0,get:function(){return $.TIntersection}});Object.defineProperty(D,"array",{enumerable:!0,get:function(){return $.array}});Object.defineProperty(D,"enumlit",{enumerable:!0,get:function(){return $.enumlit}});Object.defineProperty(D,"enumtype",{enumerable:!0,get:function(){return $.enumtype}});Object.defineProperty(D,"func",{enumerable:!0,get:function(){return $.func}});Object.defineProperty(D,"iface",{enumerable:!0,get:function(){return $.iface}});Object.defineProperty(D,"lit",{enumerable:!0,get:function(){return $.lit}});Object.defineProperty(D,"name",{enumerable:!0,get:function(){return $.name}});Object.defineProperty(D,"opt",{enumerable:!0,get:function(){return $.opt}});Object.defineProperty(D,"param",{enumerable:!0,get:function(){return $.param}});Object.defineProperty(D,"tuple",{enumerable:!0,get:function(){return $.tuple}});Object.defineProperty(D,"union",{enumerable:!0,get:function(){return $.union}});Object.defineProperty(D,"intersection",{enumerable:!0,get:function(){return $.intersection}});Object.defineProperty(D,"BasicType",{enumerable:!0,get:function(){return $.BasicType}});var gc=fn();Object.defineProperty(D,"VError",{enumerable:!0,get:function(){return gc.VError}});function kc(){for(var e=[],n=0;n<arguments.length;n++)e[n]=arguments[n];for(var r=Object.assign.apply(Object,mc([{},Tt.basicTypes],e)),o={},i=0,c=e;i<c.length;i++)for(var p=c[i],h=0,g=Object.keys(p);h<g.length;h++){var y=g[h];o[y]=new Qo(r,p[y])}return o}D.createCheckers=kc;var Qo=(function(){function e(n,r,o){if(o===void 0&&(o="value"),this.suite=n,this.ttype=r,this._path=o,this.props=new Map,r instanceof Tt.TIface)for(var i=0,c=r.props;i<c.length;i++){var p=c[i];this.props.set(p.name,p.ttype)}this.checkerPlain=this.ttype.getChecker(n,!1),this.checkerStrict=this.ttype.getChecker(n,!0)}return e.prototype.setReportedPath=function(n){this._path=n},e.prototype.check=function(n){return this._doCheck(this.checkerPlain,n)},e.prototype.test=function(n){return this.checkerPlain(n,new ot.NoopContext)},e.prototype.validate=function(n){return this._doValidate(this.checkerPlain,n)},e.prototype.strictCheck=function(n){return this._doCheck(this.checkerStrict,n)},e.prototype.strictTest=function(n){return this.checkerStrict(n,new ot.NoopContext)},e.prototype.strictValidate=function(n){return this._doValidate(this.checkerStrict,n)},e.prototype.getProp=function(n){var r=this.props.get(n);if(!r)throw new Error("Type has no property "+n);return new e(this.suite,r,this._path+"."+n)},e.prototype.methodArgs=function(n){var r=this._getMethod(n);return new e(this.suite,r.paramList)},e.prototype.methodResult=function(n){var r=this._getMethod(n);return new e(this.suite,r.result)},e.prototype.getArgs=function(){if(!(this.ttype instanceof Tt.TFunc))throw new Error("getArgs() applied to non-function");return new e(this.suite,this.ttype.paramList)},e.prototype.getResult=function(){if(!(this.ttype instanceof Tt.TFunc))throw new Error("getResult() applied to non-function");return new e(this.suite,this.ttype.result)},e.prototype.getType=function(){return this.ttype},e.prototype._doCheck=function(n,r){var o=new ot.NoopContext;if(!n(r,o)){var i=new ot.DetailContext;throw n(r,i),i.getError(this._path)}},e.prototype._doValidate=function(n,r){var o=new ot.NoopContext;if(n(r,o))return null;var i=new ot.DetailContext;return n(r,i),i.getErrorDetail(this._path)},e.prototype._getMethod=function(n){var r=this.props.get(n);if(!r)throw new Error("Type has no property "+n);if(!(r instanceof Tt.TFunc))throw new Error("Property "+n+" is not a method");return r},e})();D.Checker=Qo});var M1=en(Zt=>{"use strict";Zt.__esModule=!0;Zt.LinesAndColumns=void 0;var Fn=`
`,D1="\r",O1=(function(){function e(n){this.string=n;for(var r=[0],o=0;o<n.length;)switch(n[o]){case Fn:o+=Fn.length,r.push(o);break;case D1:o+=D1.length,n[o]===Fn&&(o+=Fn.length),r.push(o);break;default:o++;break}this.offsets=r}return e.prototype.locationForIndex=function(n){if(n<0||n>this.string.length)return null;for(var r=0,o=this.offsets;o[r+1]<=n;)r++;var i=n-o[r];return{line:r,column:i}},e.prototype.indexForLocation=function(n){var r=n.line,o=n.column;return r<0||r>=this.offsets.length||o<0||o>this.lengthOfLine(r)?null:this.offsets[r]+o},e.prototype.lengthOfLine=function(n){var r=this.offsets[n],o=n===this.offsets.length-1?this.string.length:this.offsets[n+1];return o-r},e})();Zt.LinesAndColumns=O1;Zt.default=O1});var po=`"use strict";
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
  var REPLACEMENT = 65533;
  var TextEncoderPolyfill = class {
    encoding = "utf-8";
    encode(text = "") {
      const input = String(text);
      const out = new Uint8Array(input.length * 3);
      let length = 0;
      for (let index = 0; index < input.length; index += 1) {
        let code = input.charCodeAt(index);
        if (code >= 55296 && code <= 56319 && index + 1 < input.length) {
          const next = input.charCodeAt(index + 1);
          if (next >= 56320 && next <= 57343) {
            code = 65536 + (code - 55296 << 10) + (next - 56320);
            index += 1;
          }
        }
        if (code >= 55296 && code <= 57343) code = REPLACEMENT;
        if (code < 128) {
          out[length++] = code;
        } else if (code < 2048) {
          out[length++] = 192 | code >> 6;
          out[length++] = 128 | code & 63;
        } else if (code < 65536) {
          out[length++] = 224 | code >> 12;
          out[length++] = 128 | code >> 6 & 63;
          out[length++] = 128 | code & 63;
        } else {
          out[length++] = 240 | code >> 18;
          out[length++] = 128 | code >> 12 & 63;
          out[length++] = 128 | code >> 6 & 63;
          out[length++] = 128 | code & 63;
        }
      }
      return out.slice(0, length);
    }
  };
  var DECODE_CHUNK = 8192;
  var continues = (byte) => byte !== void 0 && (byte & 192) === 128;
  var TextDecoderPolyfill = class {
    encoding = "utf-8";
    fatal;
    constructor(label = "utf-8", options = {}) {
      if (!["utf-8", "utf8", "unicode-1-1-utf-8"].includes(String(label).trim().toLowerCase())) {
        throw new RangeError(\`Raygun's TextDecoder reads UTF-8, not \${String(label)}\`);
      }
      this.fatal = options.fatal === true;
    }
    decode(input) {
      if (!input) return "";
      const bytes = input instanceof ArrayBuffer ? new Uint8Array(input) : new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
      const pieces = [];
      const units = [];
      const flush = () => {
        pieces.push(String.fromCharCode.apply(null, units));
        units.length = 0;
      };
      let index = 0;
      if (bytes[0] === 239 && bytes[1] === 187 && bytes[2] === 191) index = 3;
      while (index < bytes.length) {
        const first = bytes[index];
        let code = -1;
        let size = 1;
        if (first < 128) {
          code = first;
        } else if (first >= 194 && first <= 223 && continues(bytes[index + 1])) {
          code = (first & 31) << 6 | bytes[index + 1] & 63;
          size = 2;
        } else if (first >= 224 && first <= 239 && continues(bytes[index + 1]) && continues(bytes[index + 2])) {
          code = (first & 15) << 12 | (bytes[index + 1] & 63) << 6 | bytes[index + 2] & 63;
          size = 3;
          if (code < 2048 || code >= 55296 && code <= 57343) code = -1;
        } else if (first >= 240 && first <= 244 && continues(bytes[index + 1]) && continues(bytes[index + 2]) && continues(bytes[index + 3])) {
          code = (first & 7) << 18 | (bytes[index + 1] & 63) << 12 | (bytes[index + 2] & 63) << 6 | bytes[index + 3] & 63;
          size = 4;
          if (code < 65536 || code > 1114111) code = -1;
        }
        if (code < 0) {
          if (this.fatal) throw new TypeError("The encoded data was not valid UTF-8");
          code = REPLACEMENT;
          size = 1;
        }
        index += size;
        if (code >= 65536) {
          code -= 65536;
          units.push(55296 + (code >> 10), 56320 + (code & 1023));
        } else {
          units.push(code);
        }
        if (units.length >= DECODE_CHUNK) flush();
      }
      flush();
      return pieces.length === 1 ? pieces[0] : pieces.join("");
    }
  };
  var BASE64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  var bytesToBase64 = (bytes) => {
    const pieces = [];
    let piece = "";
    for (let index = 0; index < bytes.length; index += 3) {
      const a = bytes[index];
      const b = bytes[index + 1];
      const c = bytes[index + 2];
      const triple = a << 16 | (b ?? 0) << 8 | (c ?? 0);
      piece += BASE64[triple >> 18 & 63];
      piece += BASE64[triple >> 12 & 63];
      piece += b === void 0 ? "=" : BASE64[triple >> 6 & 63];
      piece += c === void 0 ? "=" : BASE64[triple & 63];
      if (piece.length >= DECODE_CHUNK) {
        pieces.push(piece);
        piece = "";
      }
    }
    pieces.push(piece);
    return pieces.join("");
  };
  var base64ToBytes = (base64) => {
    const clean = base64.replace(/[^A-Za-z0-9+/]/g, "");
    const out = new Uint8Array(Math.floor(clean.length * 3 / 4));
    let length = 0;
    for (let index = 0; index < clean.length; index += 4) {
      const values = [0, 1, 2, 3].map((offset) => BASE64.indexOf(clean[index + offset] ?? "A"));
      const triple = (values[0] ?? 0) << 18 | (values[1] ?? 0) << 12 | (values[2] ?? 0) << 6 | (values[3] ?? 0);
      out[length++] = triple >> 16 & 255;
      if (index + 2 < clean.length) out[length++] = triple >> 8 & 255;
      if (index + 3 < clean.length) out[length++] = triple & 255;
    }
    return out.slice(0, length);
  };
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
  var bufferOf = (bytes) => bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
  var ResponsePolyfill = class _ResponsePolyfill {
    status;
    statusText;
    headers;
    url;
    #text;
    #bytes;
    bodyUsed = false;
    constructor(body, init = {}) {
      if (typeof body === "string") {
        this.#text = body;
        this.#bytes = null;
      } else {
        this.#text = null;
        this.#bytes = body === null || body === void 0 ? new Uint8Array() : typeof body === "object" && "pipeThrough" in body ? body.bytes : toBytes(body);
      }
      this.status = init.status ?? 200;
      this.statusText = init.statusText ?? "";
      this.headers = new HeadersPolyfill(init.headers);
      this.url = init.url ?? "";
    }
    get ok() {
      return this.status >= 200 && this.status < 300;
    }
    /** The body's bytes, made from its text the first time they're asked for. */
    #binary() {
      if (this.#bytes === null) this.#bytes = new TextEncoderPolyfill().encode(this.#text ?? "");
      return this.#bytes;
    }
    async text() {
      this.bodyUsed = true;
      return this.#text ?? new TextDecoderPolyfill().decode(this.#binary());
    }
    async json() {
      return JSON.parse(await this.text());
    }
    async arrayBuffer() {
      this.bodyUsed = true;
      return bufferOf(this.#binary());
    }
    async bytes() {
      this.bodyUsed = true;
      return this.#binary().slice();
    }
    async blob() {
      this.bodyUsed = true;
      return new BlobPolyfill([this.#binary()], { type: this.headers.get("content-type") ?? "" });
    }
    clone() {
      return new _ResponsePolyfill(this.#text ?? this.#bytes, this);
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
  var inert = new Proxy(function inert2() {
  }, {
    get: (_target, property) => {
      if (property === Symbol.toPrimitive) return () => "";
      if (property === Symbol.iterator) return function* empty() {
      };
      if (property === "then") return void 0;
      return inert;
    },
    set: () => true,
    apply: () => inert,
    construct: () => inert
  });
  var inertElement = () => {
    const element = {
      // Styles are set and read back as nothing.
      style: inert,
      dataset: {},
      children: [],
      childNodes: [],
      innerHTML: "",
      textContent: "",
      classList: { add() {
      }, remove() {
      }, toggle: () => false, contains: () => false, replace() {
      } },
      appendChild: (child) => child,
      removeChild: (child) => child,
      insertBefore: (child) => child,
      append() {
      },
      prepend() {
      },
      remove() {
      },
      setAttribute() {
      },
      getAttribute: () => null,
      removeAttribute() {
      },
      addEventListener() {
      },
      removeEventListener() {
      },
      querySelector: () => inertElement(),
      querySelectorAll: () => [],
      getBoundingClientRect: () => ({ top: 0, left: 0, width: 0, height: 0, right: 0, bottom: 0 })
    };
    return new Proxy(element, {
      get: (target, property) => property in target ? target[property] : inert
    });
  };
  var inertDocument = () => new Proxy(
    {
      head: inertElement(),
      body: inertElement(),
      documentElement: inertElement(),
      createElement: () => inertElement(),
      createElementNS: () => inertElement(),
      createTextNode: () => inertElement(),
      // A plugin looking up the elements it drew finds them (inert), as it would in Nuclear.
      getElementById: () => inertElement(),
      querySelector: () => inertElement(),
      getElementsByTagName: () => [],
      getElementsByClassName: () => [],
      querySelectorAll: () => [],
      addEventListener() {
      },
      removeEventListener() {
      }
    },
    { get: (target, property) => property in target ? target[property] : inert }
  );
  var inertNavigator = () => new Proxy(
    { userAgent: "Mozilla/5.0", language: "en-US", languages: ["en-US"], onLine: true },
    {
      get: (target, property) => property in target ? target[property] : inert
    }
  );
  var installPolyfills = (target = globalThis) => {
    const provide2 = (name, value) => {
      if (target[name] === void 0) target[name] = value;
    };
    provide2("TextEncoder", TextEncoderPolyfill);
    provide2("TextDecoder", TextDecoderPolyfill);
    provide2("atob", atobPolyfill);
    provide2("btoa", btoaPolyfill);
    provide2("URL", URLPolyfill);
    provide2("URLSearchParams", URLSearchParamsPolyfill);
    provide2("Headers", HeadersPolyfill);
    provide2("Blob", BlobPolyfill);
    provide2("DecompressionStream", DecompressionStreamPolyfill);
    provide2("Response", ResponsePolyfill);
    provide2("setTimeout", setTimeoutPolyfill);
    provide2("clearTimeout", clearTimeoutPolyfill);
    provide2("setInterval", setIntervalPolyfill);
    provide2("clearInterval", clearTimeoutPolyfill);
    provide2("AbortController", AbortControllerPolyfill);
    provide2("queueMicrotask", (callback) => void Promise.resolve().then(callback));
    const started = Date.now();
    provide2("performance", { now: () => Date.now() - started, timeOrigin: started });
    provide2("document", inertDocument());
    provide2("navigator", inertNavigator());
    provide2("window", target);
    provide2("addEventListener", () => {
    });
    provide2("removeEventListener", () => {
    });
    provide2("dispatchEvent", () => true);
    provide2("requestAnimationFrame", () => 0);
    provide2("cancelAnimationFrame", () => {
    });
    provide2("matchMedia", (query) => ({
      matches: false,
      media: String(query),
      addEventListener() {
      },
      removeEventListener() {
      },
      addListener() {
      },
      removeListener() {
      }
    }));
    provide2("getComputedStyle", () => ({ getPropertyValue: () => "" }));
    for (const name of ["MutationObserver", "ResizeObserver", "IntersectionObserver"]) {
      provide2(name, InertObserver);
    }
  };
  var InertObserver = class {
    observe() {
    }
    unobserve() {
    }
    disconnect() {
    }
    takeRecords() {
      return [];
    }
  };

  // src/shim/ytdlp.ts
  var KEPT = 500;
  var keep = (table, key, value) => {
    table.delete(key);
    table.set(key, value);
    if (table.size > KEPT) table.delete(table.keys().next().value);
  };
  var channels = /* @__PURE__ */ new Map();
  var wishes = /* @__PURE__ */ new Map();
  var streams = /* @__PURE__ */ new Map();
  var authors = /* @__PURE__ */ new Map();
  var videoIdOf = (text) => {
    const trimmed = (text ?? "").trim();
    if (/^[\\w-]{11}$/.test(trimmed)) return trimmed;
    return /(?:[?&]v=|youtu\\.be\\/|\\/(?:shorts|live|embed|v)\\/)([\\w-]{11})(?![\\w-])/.exec(trimmed)?.[1] ?? null;
  };
  var channelOf = (video) => {
    const id = videoIdOf(video);
    return id ? channels.get(id) ?? null : null;
  };
  var wishFor = (video, wish) => {
    const id = videoIdOf(video);
    if (id) keep(wishes, id, wish);
  };
  var streamAt = (url) => streams.get(url) ?? null;
  var playlistAuthorOf = (id) => id ? authors.get(id) ?? null : null;
  var seconds = (ms) => ms == null ? null : ms / 1e3;
  var createYtdlp = () => ({
    async search(query, maxResults) {
      const found = await raygun.youtube.search(String(query), maxResults);
      return found.map((video) => {
        if (video.channel) keep(channels, video.id, video.channel);
        return {
          id: video.id,
          title: video.title,
          duration: seconds(video.durationMs),
          thumbnail: video.thumbnailUrl,
          channel: video.channel
        };
      });
    },
    async getStream(url) {
      const id = videoIdOf(url);
      const stream = await raygun.youtube.stream(String(url), id && wishes.get(id) || {});
      keep(streams, stream.url, stream);
      return {
        stream_url: stream.url,
        duration: seconds(stream.durationMs),
        title: stream.title,
        container: stream.container,
        codec: stream.codec,
        album: stream.album,
        artists: stream.artists,
        album_artists: [],
        upload_date: stream.uploadDate
      };
    },
    async getPlaylist(url) {
      const playlist = await raygun.youtube.playlist(String(url));
      if (playlist.author) keep(authors, playlist.id, playlist.author);
      return {
        id: playlist.id,
        title: playlist.title,
        entries: playlist.videos.map((video) => {
          if (video.channel) keep(channels, video.id, video.channel);
          return {
            id: video.id,
            title: video.title,
            duration: seconds(video.durationMs),
            thumbnails: video.thumbnails,
            channel: video.channel
          };
        })
      };
    }
  });

  // src/shim/providers.ts
  var KIND_MAP = {
    metadata: "music.metadata",
    streaming: "music.stream",
    lyrics: "lyrics",
    dashboard: "music.charts",
    discovery: "music.recommend",
    playlists: "music.metadata"
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
  var KEPT_CANDIDATES = 500;
  var keepCandidate = (candidate) => {
    candidates.delete(candidate.id);
    candidates.set(candidate.id, candidate);
    if (candidates.size > KEPT_CANDIDATES) {
      candidates.delete(candidates.keys().next().value);
    }
  };
  var sourceOf = (url) => ({
    kind: "url",
    url,
    headers: streamAt(url)?.headers ?? {}
  });
  var songItems = (playlist) => playlist.items.filter((item) => typeof item?.track?.title === "string");
  var toRaygunPlaylist = (url, playlist) => ({
    id: url,
    title: playlist.name,
    author: playlistAuthorOf(playlist.origin?.id),
    artworkUrl: pickArtwork(playlist.artwork) ?? songItems(playlist).map((item) => pickArtwork(item.track.artwork)).find(Boolean) ?? null,
    trackCount: songItems(playlist).length,
    featured: false
  });
  var PLAYLIST_REUSE_MS = 5 * 60 * 1e3;
  var wrapPlaylists = (provider, id, name) => {
    if (!has(provider, "fetchPlaylistByUrl")) return null;
    const read = /* @__PURE__ */ new Map();
    const matches = (url) => {
      if (!/^https?:\\/\\/\\S+$/i.test(url)) return false;
      if (!has(provider, "matchesUrl")) return true;
      try {
        return Boolean(provider.matchesUrl.call(provider, url));
      } catch {
        return false;
      }
    };
    const playlistAt = async (url) => {
      const kept = read.get(url);
      if (kept && Date.now() - kept.at < PLAYLIST_REUSE_MS) return kept.playlist;
      const playlist = await call(provider, "fetchPlaylistByUrl", url);
      if (!playlist || !Array.isArray(playlist.items)) {
        throw new Error(\`\${name} found no playlist there\`);
      }
      read.delete(url);
      read.set(url, { at: Date.now(), playlist });
      if (read.size > 20) read.delete(read.keys().next().value);
      return playlist;
    };
    const songs = (playlist) => songItems(playlist).map((item) => toRaygunTrack(item.track));
    return {
      kind: "music.metadata",
      id,
      name,
      async search({ query, limit }) {
        const url = query.trim();
        if (!matches(url)) return { tracks: [] };
        return { tracks: songs(await playlistAt(url)).slice(0, limit) };
      },
      async searchPlaylists({ query }) {
        const url = query.trim();
        if (!matches(url)) return { playlists: [] };
        return { playlists: [toRaygunPlaylist(url, await playlistAt(url))] };
      },
      async playlist({ id: url }) {
        if (!matches(url)) throw new Error(\`\${name} doesn't open \${url}\`);
        const playlist = await playlistAt(url);
        return { playlist: toRaygunPlaylist(url, playlist), tracks: songs(playlist) };
      }
    };
  };
  var wrapProvider = (provider) => {
    const kind = KIND_MAP[provider.kind];
    const id = providerId(provider.id);
    const name = provider.name;
    if (provider.kind === "playlists") return wrapPlaylists(provider, id, name);
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
          for (const candidate of list) keepCandidate(candidate);
          return {
            candidates: list.map((candidate) => {
              const artist = channelOf(candidate.id) ?? channelOf(candidate.source?.id);
              return {
                id: candidate.id,
                title: candidate.title,
                ...artist ? { artist } : {},
                durationMs: candidate.durationMs ?? null,
                quality: candidate.stream?.qualityLabel ?? null,
                source: playable(candidate.stream) ? sourceOf(candidate.stream.url) : null
              };
            })
          };
        },
        async resolve({ id: candidateId, targetKbps, decodable }) {
          const known = candidates.get(candidateId);
          const wish = { targetKbps: targetKbps ?? null, decodable: decodable === true };
          wishFor(candidateId, wish);
          wishFor(known?.source?.id, wish);
          const stream = has(provider, "getStreamUrlV2") && known ? await call(provider, "getStreamUrlV2", known) : await call(provider, "getStreamUrl", candidateId);
          if (!playable(stream)) {
            throw new Error(
              \`\${name} returned a \${stream?.protocol ?? "missing"} stream Raygun can't play\`
            );
          }
          const expiresAt = streamAt(stream.url)?.expiresAt;
          return { source: sourceOf(stream.url), ...expiresAt ? { expiresAt } : {} };
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
    if (kind === "music.charts") {
      return {
        kind,
        id,
        name,
        async charts() {
          if (!has(provider, "fetchTopTracks")) return { charts: [] };
          return {
            charts: [
              {
                id: "topTracks",
                name: \`Top songs on \${name}\`,
                description: \`\${name}'s most played songs\`,
                group: "top"
              }
            ]
          };
        },
        async chart({ id: chart, limit }) {
          if (chart !== "topTracks") throw new Error(\`Unknown chart \${chart}\`);
          const tracks = await call(provider, "fetchTopTracks");
          return { tracks: (tracks ?? []).slice(0, limit).map(toRaygunTrack) };
        }
      };
    }
    if (kind === "music.recommend") {
      return {
        kind,
        id,
        name,
        async radio({ artist, title, limit }) {
          const context = [
            toNuclearTrack({ title: title ?? "", artist, album: null, durationMs: null })
          ];
          const tracks = await call(provider, "getRecommendations", context, {
            variety: 0.5,
            limit
          });
          return { tracks: (tracks ?? []).slice(0, limit).map(toRaygunTrack) };
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
    "Metadata",
    "Streaming",
    "Lyrics"
  ];
  var lenient = (domain, object) => {
    const warned = /* @__PURE__ */ new Set();
    return new Proxy(object, {
      get: (target, property, receiver) => {
        if (property in target || typeof property === "symbol" || property === "then") {
          return Reflect.get(target, property, receiver);
        }
        return () => {
          if (!warned.has(property)) {
            warned.add(property);
            raygun.log.warn(\`\${domain}.\${property} isn't in Raygun's Nuclear API; skipped\`);
          }
          return void 0;
        };
      }
    });
  };
  var LISTENS = /^(on|once|off|subscribe|unsubscribe|add\\w*Listener|remove\\w*Listener|listen)/;
  var unsupported = (domain) => new Proxy(
    {},
    {
      get: (_target, property) => {
        if (property === Symbol.toPrimitive || property === "then") return void 0;
        if (typeof property === "string" && LISTENS.test(property)) {
          return () => () => {
          };
        }
        return () => Promise.reject(new Error(\`\${domain} isn't available to Nuclear plugins in Raygun yet\`));
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
  var requestBody = (body) => {
    if (body === null || body === void 0) return {};
    if (typeof body === "string") return { body };
    if (body instanceof URLSearchParamsPolyfill) {
      return { body: body.toString(), type: "application/x-www-form-urlencoded;charset=UTF-8" };
    }
    if (body instanceof BlobPolyfill) {
      return { bodyBase64: bytesToBase64(body.bytes), type: body.type || void 0 };
    }
    if (body instanceof ArrayBuffer) return { bodyBase64: bytesToBase64(new Uint8Array(body)) };
    if (ArrayBuffer.isView(body)) {
      return {
        bodyBase64: bytesToBase64(new Uint8Array(body.buffer, body.byteOffset, body.byteLength))
      };
    }
    return { body: String(body) };
  };
  var pluginFetch = async (input, init = {}) => {
    const request = typeof input === "object" && input !== null && "url" in input ? input : null;
    const url = String(request ? request.url : input);
    const headers = headersRecord(init.headers ?? request?.headers);
    const { body, bodyBase64, type } = requestBody(init.body ?? request?.body);
    if (type && !Object.keys(headers).some((name) => name.toLowerCase() === "content-type")) {
      headers["Content-Type"] = type;
    }
    const response = await raygun.http.fetch(url, {
      method: init.method ?? request?.method ?? "GET",
      headers,
      body: body ?? null,
      ...bodyBase64 === void 0 ? {} : { bodyBase64 },
      // Bytes that aren't text (a picture, a zip) come as they are; text comes as text.
      binary: true
    });
    const bytes = typeof response.bytes === "function" ? await response.bytes() : null;
    return new ResponsePolyfill(bytes ?? await response.text(), {
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
      },
      // A settings widget (Subsonic's connection test) is Nuclear UI; Raygun shows the fields only.
      registerWidget() {
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
          try {
            raygun.providers.register(wrapped);
          } catch (error) {
            Logger.warn(\`\${plugin.name}: skipped a "\${provider.kind}" provider: \${String(error)}\`);
          }
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
      Settings: lenient("Settings", Settings),
      Providers: lenient("Providers", Providers),
      Ytdlp: lenient("Ytdlp", createYtdlp())
    };
    for (const domain of STUBBED_DOMAINS) api[domain] = unsupported(domain);
    return api;
  };

  // src/shim/native.ts
  var unwrap = (text) => {
    const envelope = JSON.parse(text);
    if (envelope.error !== void 0) throw new Error(envelope.error);
    return envelope.value;
  };
  var takeNatives = (scope2) => {
    const natives2 = scope2.__rgNuclear ?? null;
    delete scope2.__rgNuclear;
    return natives2;
  };

  // src/shim/crypto.ts
  var rawKeys = /* @__PURE__ */ new WeakMap();
  var MAX_RANDOM_BYTES = 65536;
  var nameOf = (algorithm, fallback) => algorithm === void 0 ? fallback : typeof algorithm === "string" ? algorithm : algorithm.name;
  var bytesOf = (data) => {
    if (data instanceof ArrayBuffer) return new Uint8Array(data);
    if (ArrayBuffer.isView(data))
      return new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
    throw new TypeError("Expected an ArrayBuffer or a typed array");
  };
  var bufferOf2 = (bytes) => bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
  var createCrypto = (natives2) => {
    const run2 = (args) => base64ToBytes(unwrap(natives2.crypto(JSON.stringify(args))));
    const random = (count) => base64ToBytes(unwrap(natives2.random(count)));
    const sign = (key, data) => {
      const raw = rawKeys.get(key);
      if (!raw) throw new TypeError("Not a key made by crypto.subtle.importKey");
      return run2({
        op: "hmac",
        algorithm: key.algorithm.hash.name,
        key: bytesToBase64(raw),
        data: bytesToBase64(bytesOf(data))
      });
    };
    return {
      getRandomValues(array) {
        if (array.byteLength > MAX_RANDOM_BYTES) {
          throw new RangeError(\`getRandomValues takes at most \${MAX_RANDOM_BYTES} bytes\`);
        }
        new Uint8Array(array.buffer, array.byteOffset, array.byteLength).set(
          random(array.byteLength)
        );
        return array;
      },
      randomUUID() {
        const bytes = random(16);
        bytes[6] = bytes[6] & 15 | 64;
        bytes[8] = bytes[8] & 63 | 128;
        const hex = Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
        return \`\${hex.slice(0, 8)}-\${hex.slice(8, 12)}-\${hex.slice(12, 16)}-\${hex.slice(16, 20)}-\${hex.slice(20)}\`;
      },
      subtle: {
        async digest(algorithm, data) {
          return bufferOf2(
            run2({
              op: "digest",
              algorithm: nameOf(algorithm, "SHA-256"),
              data: bytesToBase64(bytesOf(data))
            })
          );
        },
        async importKey(format2, keyData, algorithm, extractable = false, usages = []) {
          const name = nameOf(algorithm, "HMAC");
          if (format2 !== "raw" || name.toUpperCase() !== "HMAC") {
            throw new Error(\`Raygun's crypto imports raw HMAC keys only, not \${format2} \${name}\`);
          }
          const hash = typeof algorithm === "string" ? void 0 : algorithm.hash;
          const key = {
            type: "secret",
            extractable,
            algorithm: { name: "HMAC", hash: { name: nameOf(hash, "SHA-256") } },
            usages
          };
          rawKeys.set(key, bytesOf(keyData).slice());
          return key;
        },
        async sign(_algorithm, key, data) {
          return bufferOf2(sign(key, data));
        },
        async verify(_algorithm, key, signature, data) {
          const expected = sign(key, data);
          const given = bytesOf(signature);
          return expected.length === given.length && expected.every((byte, index) => byte === given[index]);
        }
      }
    };
  };

  // src/shim/dom.ts
  var listOf = (nodes) => Object.assign(nodes, { item: (index) => nodes[index] ?? null });
  var ELEMENT_NODE = 1;
  var TEXT_NODE = 3;
  var COMMENT_NODE = 8;
  var DOCUMENT_NODE = 9;
  var datasetKey = (name) => name.slice("data-".length).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());
  var createDomParser = (natives2) => {
    const run2 = (op, args) => unwrap(natives2.dom(op, JSON.stringify(args)));
    class DomNode {
      #handle;
      #info = null;
      constructor(handle) {
        this.#handle = handle;
      }
      get #meta() {
        this.#info ??= run2("info", { nodes: [this.#handle] })[0] ?? { type: "directive" };
        return this.#info;
      }
      #nodes(op, args = {}) {
        return run2(op, { nodes: [this.#handle], ...args }).map(wrap);
      }
      get #isElement() {
        const { type } = this.#meta;
        return type === "tag" || type === "script" || type === "style";
      }
      get nodeType() {
        const { type } = this.#meta;
        if (this.#isElement) return ELEMENT_NODE;
        if (type === "text") return TEXT_NODE;
        if (type === "comment") return COMMENT_NODE;
        return DOCUMENT_NODE;
      }
      get localName() {
        return this.#isElement ? (this.#meta.name ?? "").toLowerCase() : null;
      }
      get tagName() {
        return this.#isElement ? (this.#meta.name ?? "").toUpperCase() : void 0;
      }
      get nodeName() {
        if (this.#isElement) return this.tagName;
        const { type } = this.#meta;
        return type === "text" ? "#text" : type === "comment" ? "#comment" : "#document";
      }
      get textContent() {
        const { type, data } = this.#meta;
        if (type === "text" || type === "comment") return data ?? "";
        return run2("text", { nodes: [this.#handle] });
      }
      get innerText() {
        return this.textContent;
      }
      get data() {
        return this.#meta.data ?? "";
      }
      get nodeValue() {
        const { type, data } = this.#meta;
        return type === "text" || type === "comment" ? data ?? "" : null;
      }
      get innerHTML() {
        return run2("html", { nodes: [this.#handle] }) ?? "";
      }
      get outerHTML() {
        return run2("outer", { nodes: [this.#handle] });
      }
      getAttribute(name) {
        return this.#meta.attribs?.[String(name).toLowerCase()] ?? null;
      }
      hasAttribute(name) {
        return this.getAttribute(name) !== null;
      }
      getAttributeNames() {
        return Object.keys(this.#meta.attribs ?? {});
      }
      get attributes() {
        return Object.entries(this.#meta.attribs ?? {}).map(([name, value]) => ({ name, value }));
      }
      get id() {
        return this.getAttribute("id") ?? "";
      }
      get className() {
        return this.getAttribute("class") ?? "";
      }
      get classList() {
        const names = this.className.split(/\\s+/).filter(Boolean);
        return Object.assign([...names], {
          contains: (name) => names.includes(name),
          item: (index) => names[index] ?? null
        });
      }
      get dataset() {
        return Object.fromEntries(
          Object.entries(this.#meta.attribs ?? {}).filter(([name]) => name.startsWith("data-")).map(([name, value]) => [datasetKey(name), value])
        );
      }
      // Links and pictures as written; a parsed page has no address to resolve them against.
      get href() {
        return this.getAttribute("href") ?? "";
      }
      get src() {
        return this.getAttribute("src") ?? "";
      }
      get title() {
        return this.getAttribute("title") ?? "";
      }
      get value() {
        return this.getAttribute("value") ?? "";
      }
      /** Matches below this node (never the node itself, as the DOM has it). */
      querySelectorAll(selector) {
        const handles = run2("select", {
          nodes: [this.#handle],
          selector: String(selector)
        });
        return listOf(handles.filter((handle) => handle !== this.#handle).map(wrap));
      }
      querySelector(selector) {
        return this.querySelectorAll(selector)[0] ?? null;
      }
      getElementsByTagName(name) {
        return this.querySelectorAll(name === "*" ? "*" : String(name));
      }
      getElementsByClassName(names) {
        const classes = String(names).split(/\\s+/).filter(Boolean);
        return this.querySelectorAll(classes.map((name) => \`.\${name}\`).join(""));
      }
      getElementById(id) {
        return this.querySelector(\`[id="\${String(id).replace(/["\\\\]/g, "\\\\$&")}"]\`);
      }
      matches(selector) {
        return run2("filter", { nodes: [this.#handle], selector: String(selector) }).length > 0;
      }
      closest(selector) {
        if (this.#isElement && this.matches(selector)) return this;
        const ancestors = run2("parents", { nodes: [this.#handle] });
        const matching = run2("filter", { nodes: ancestors, selector: String(selector) });
        return matching.length > 0 ? wrap(matching[0]) : null;
      }
      get children() {
        return listOf(this.#nodes("children"));
      }
      get childNodes() {
        return listOf(this.#nodes("children", { all: true }));
      }
      get childElementCount() {
        return this.children.length;
      }
      get firstElementChild() {
        return this.children[0] ?? null;
      }
      get lastElementChild() {
        return this.children.at(-1) ?? null;
      }
      get firstChild() {
        return this.childNodes[0] ?? null;
      }
      get lastChild() {
        return this.childNodes.at(-1) ?? null;
      }
      get parentElement() {
        return this.#nodes("parent")[0] ?? null;
      }
      get parentNode() {
        return this.parentElement;
      }
      get nextElementSibling() {
        return this.#nodes("siblings", { forward: true })[0] ?? null;
      }
      get previousElementSibling() {
        return this.#nodes("siblings", { forward: false })[0] ?? null;
      }
      get nextSibling() {
        return this.#nodes("siblings", { forward: true, any: true })[0] ?? null;
      }
      get previousSibling() {
        return this.#nodes("siblings", { forward: false, any: true })[0] ?? null;
      }
      // Pages are read here, not edited or watched.
      addEventListener() {
      }
      removeEventListener() {
      }
    }
    class DomDocument extends DomNode {
      get documentElement() {
        return this.querySelector("html") ?? this.firstElementChild;
      }
      get head() {
        return this.querySelector("head");
      }
      get body() {
        return this.querySelector("body");
      }
      get title() {
        return this.querySelector("title")?.textContent.trim() ?? "";
      }
    }
    const wrap = (handle) => new DomNode(handle);
    return class DOMParserPolyfill {
      /** Parses \`text\` as an HTML page, whatever type is asked (XML is read the same way). */
      parseFromString(text) {
        return new DomDocument(run2("load", { html: String(text ?? "") }));
      }
    };
  };

  // src/shim/run.ts
  var EXTENSIONS = ["", ".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs", ".json"];
  var dirname = (path) => path.includes("/") ? path.replace(/\\/[^/]*$/, "") : "";
  var join = (base, relative) => {
    const parts = [];
    for (const segment of \`\${base}/\${relative}\`.split("/")) {
      if (segment === "..") parts.pop();
      else if (segment && segment !== ".") parts.push(segment);
    }
    return parts.join("/");
  };
  var createRequire = (definitions, bare) => {
    const cache = /* @__PURE__ */ new Map();
    const load = (path) => {
      const cached = cache.get(path);
      if (cached) return cached.exports;
      const module = { exports: {} };
      cache.set(path, module);
      const define = definitions[path];
      const scope2 = globalThis;
      define(
        module.exports,
        (id) => requireFrom(path, id),
        module,
        scope2.document,
        scope2.window ?? scope2
      );
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
  var scope = globalThis;
  var natives = takeNatives(scope);
  installPolyfills(scope);
  var provide = (name, value) => {
    if (scope[name] === void 0) scope[name] = value;
  };
  provide("fetch", pluginFetch);
  if (natives) {
    provide("DOMParser", createDomParser(natives));
    provide("crypto", createCrypto(natives));
  }
  return __toCommonJS(index_exports);
})();
`;var l;(function(e){e[e.NONE=0]="NONE";let r=1;e[e._abstract=r]="_abstract";let o=r+1;e[e._accessor=o]="_accessor";let i=o+1;e[e._as=i]="_as";let c=i+1;e[e._assert=c]="_assert";let p=c+1;e[e._asserts=p]="_asserts";let h=p+1;e[e._async=h]="_async";let g=h+1;e[e._await=g]="_await";let y=g+1;e[e._checks=y]="_checks";let x=y+1;e[e._constructor=x]="_constructor";let v=x+1;e[e._declare=v]="_declare";let T=v+1;e[e._enum=T]="_enum";let E=T+1;e[e._exports=E]="_exports";let O=E+1;e[e._from=O]="_from";let q=O+1;e[e._get=q]="_get";let G=q+1;e[e._global=G]="_global";let N=G+1;e[e._implements=N]="_implements";let J=N+1;e[e._infer=J]="_infer";let ae=J+1;e[e._interface=ae]="_interface";let ce=ae+1;e[e._is=ce]="_is";let je=ce+1;e[e._keyof=je]="_keyof";let Fe=je+1;e[e._mixins=Fe]="_mixins";let Be=Fe+1;e[e._module=Be]="_module";let qe=Be+1;e[e._namespace=qe]="_namespace";let $e=qe+1;e[e._of=$e]="_of";let Ue=$e+1;e[e._opaque=Ue]="_opaque";let He=Ue+1;e[e._out=He]="_out";let Ve=He+1;e[e._override=Ve]="_override";let We=Ve+1;e[e._private=We]="_private";let ze=We+1;e[e._protected=ze]="_protected";let Xe=ze+1;e[e._proto=Xe]="_proto";let Je=Xe+1;e[e._public=Je]="_public";let Ge=Je+1;e[e._readonly=Ge]="_readonly";let Ke=Ge+1;e[e._require=Ke]="_require";let Ye=Ke+1;e[e._satisfies=Ye]="_satisfies";let Qe=Ye+1;e[e._set=Qe]="_set";let Ze=Qe+1;e[e._static=Ze]="_static";let et=Ze+1;e[e._symbol=et]="_symbol";let tt=et+1;e[e._type=tt]="_type";let nt=tt+1;e[e._unique=nt]="_unique";let mt=nt+1;e[e._using=mt]="_using"})(l||(l={}));var t;(function(e){e[e.PRECEDENCE_MASK=15]="PRECEDENCE_MASK";let r=16;e[e.IS_KEYWORD=r]="IS_KEYWORD";let o=32;e[e.IS_ASSIGN=o]="IS_ASSIGN";let i=64;e[e.IS_RIGHT_ASSOCIATIVE=i]="IS_RIGHT_ASSOCIATIVE";let c=128;e[e.IS_PREFIX=c]="IS_PREFIX";let p=256;e[e.IS_POSTFIX=p]="IS_POSTFIX";let h=512;e[e.IS_EXPRESSION_START=h]="IS_EXPRESSION_START";let g=512;e[e.num=g]="num";let y=1536;e[e.bigint=y]="bigint";let x=2560;e[e.decimal=x]="decimal";let v=3584;e[e.regexp=v]="regexp";let T=4608;e[e.string=T]="string";let E=5632;e[e.name=E]="name";let O=6144;e[e.eof=O]="eof";let q=7680;e[e.bracketL=q]="bracketL";let G=8192;e[e.bracketR=G]="bracketR";let N=9728;e[e.braceL=N]="braceL";let J=10752;e[e.braceBarL=J]="braceBarL";let ae=11264;e[e.braceR=ae]="braceR";let ce=12288;e[e.braceBarR=ce]="braceBarR";let je=13824;e[e.parenL=je]="parenL";let Fe=14336;e[e.parenR=Fe]="parenR";let Be=15360;e[e.comma=Be]="comma";let qe=16384;e[e.semi=qe]="semi";let $e=17408;e[e.colon=$e]="colon";let Ue=18432;e[e.doubleColon=Ue]="doubleColon";let He=19456;e[e.dot=He]="dot";let Ve=20480;e[e.question=Ve]="question";let We=21504;e[e.questionDot=We]="questionDot";let ze=22528;e[e.arrow=ze]="arrow";let Xe=23552;e[e.template=Xe]="template";let Je=24576;e[e.ellipsis=Je]="ellipsis";let Ge=25600;e[e.backQuote=Ge]="backQuote";let Ke=27136;e[e.dollarBraceL=Ke]="dollarBraceL";let Ye=27648;e[e.at=Ye]="at";let Qe=29184;e[e.hash=Qe]="hash";let Ze=29728;e[e.eq=Ze]="eq";let et=30752;e[e.assign=et]="assign";let tt=32640;e[e.preIncDec=tt]="preIncDec";let nt=33664;e[e.postIncDec=nt]="postIncDec";let mt=34432;e[e.bang=mt]="bang";let Bn=35456;e[e.tilde=Bn]="tilde";let qn=35841;e[e.pipeline=qn]="pipeline";let $n=36866;e[e.nullishCoalescing=$n]="nullishCoalescing";let Un=37890;e[e.logicalOR=Un]="logicalOR";let Hn=38915;e[e.logicalAND=Hn]="logicalAND";let Vn=39940;e[e.bitwiseOR=Vn]="bitwiseOR";let Wn=40965;e[e.bitwiseXOR=Wn]="bitwiseXOR";let zn=41990;e[e.bitwiseAND=zn]="bitwiseAND";let Xn=43015;e[e.equality=Xn]="equality";let Jn=44040;e[e.lessThan=Jn]="lessThan";let Gn=45064;e[e.greaterThan=Gn]="greaterThan";let Kn=46088;e[e.relationalOrEqual=Kn]="relationalOrEqual";let Yn=47113;e[e.bitShiftL=Yn]="bitShiftL";let Qn=48137;e[e.bitShiftR=Qn]="bitShiftR";let Zn=49802;e[e.plus=Zn]="plus";let er=50826;e[e.minus=er]="minus";let tr=51723;e[e.modulo=tr]="modulo";let nr=52235;e[e.star=nr]="star";let rr=53259;e[e.slash=rr]="slash";let sr=54348;e[e.exponent=sr]="exponent";let or=55296;e[e.jsxName=or]="jsxName";let ir=56320;e[e.jsxText=ir]="jsxText";let ar=57344;e[e.jsxEmptyText=ar]="jsxEmptyText";let cr=58880;e[e.jsxTagStart=cr]="jsxTagStart";let lr=59392;e[e.jsxTagEnd=lr]="jsxTagEnd";let ur=60928;e[e.typeParameterStart=ur]="typeParameterStart";let pr=61440;e[e.nonNullAssertion=pr]="nonNullAssertion";let fr=62480;e[e._break=fr]="_break";let hr=63504;e[e._case=hr]="_case";let dr=64528;e[e._catch=dr]="_catch";let mr=65552;e[e._continue=mr]="_continue";let gr=66576;e[e._debugger=gr]="_debugger";let kr=67600;e[e._default=kr]="_default";let yr=68624;e[e._do=yr]="_do";let xr=69648;e[e._else=xr]="_else";let br=70672;e[e._finally=br]="_finally";let _r=71696;e[e._for=_r]="_for";let wr=73232;e[e._function=wr]="_function";let vr=73744;e[e._if=vr]="_if";let Tr=74768;e[e._return=Tr]="_return";let Ir=75792;e[e._switch=Ir]="_switch";let Sr=77456;e[e._throw=Sr]="_throw";let Er=77840;e[e._try=Er]="_try";let Ar=78864;e[e._var=Ar]="_var";let Pr=79888;e[e._let=Pr]="_let";let Cr=80912;e[e._const=Cr]="_const";let Nr=81936;e[e._while=Nr]="_while";let Rr=82960;e[e._with=Rr]="_with";let Lr=84496;e[e._new=Lr]="_new";let Dr=85520;e[e._this=Dr]="_this";let Or=86544;e[e._super=Or]="_super";let Mr=87568;e[e._class=Mr]="_class";let jr=88080;e[e._extends=jr]="_extends";let Fr=89104;e[e._export=Fr]="_export";let Br=90640;e[e._import=Br]="_import";let qr=91664;e[e._yield=qr]="_yield";let $r=92688;e[e._null=$r]="_null";let Ur=93712;e[e._true=Ur]="_true";let Hr=94736;e[e._false=Hr]="_false";let Vr=95256;e[e._in=Vr]="_in";let Wr=96280;e[e._instanceof=Wr]="_instanceof";let zr=97936;e[e._typeof=zr]="_typeof";let Xr=98960;e[e._void=Xr]="_void";let q1=99984;e[e._delete=q1]="_delete";let $1=100880;e[e._async=$1]="_async";let U1=101904;e[e._get=U1]="_get";let H1=102928;e[e._set=H1]="_set";let V1=103952;e[e._declare=V1]="_declare";let W1=104976;e[e._readonly=W1]="_readonly";let z1=106e3;e[e._abstract=z1]="_abstract";let X1=107024;e[e._static=X1]="_static";let J1=107536;e[e._public=J1]="_public";let G1=108560;e[e._private=G1]="_private";let K1=109584;e[e._protected=K1]="_protected";let Y1=110608;e[e._override=Y1]="_override";let Q1=112144;e[e._as=Q1]="_as";let Z1=113168;e[e._enum=Z1]="_enum";let ea=114192;e[e._type=ea]="_type";let ta=115216;e[e._implements=ta]="_implements"})(t||(t={}));function Gr(e){switch(e){case t.num:return"num";case t.bigint:return"bigint";case t.decimal:return"decimal";case t.regexp:return"regexp";case t.string:return"string";case t.name:return"name";case t.eof:return"eof";case t.bracketL:return"[";case t.bracketR:return"]";case t.braceL:return"{";case t.braceBarL:return"{|";case t.braceR:return"}";case t.braceBarR:return"|}";case t.parenL:return"(";case t.parenR:return")";case t.comma:return",";case t.semi:return";";case t.colon:return":";case t.doubleColon:return"::";case t.dot:return".";case t.question:return"?";case t.questionDot:return"?.";case t.arrow:return"=>";case t.template:return"template";case t.ellipsis:return"...";case t.backQuote:return"`";case t.dollarBraceL:return"${";case t.at:return"@";case t.hash:return"#";case t.eq:return"=";case t.assign:return"_=";case t.preIncDec:return"++/--";case t.postIncDec:return"++/--";case t.bang:return"!";case t.tilde:return"~";case t.pipeline:return"|>";case t.nullishCoalescing:return"??";case t.logicalOR:return"||";case t.logicalAND:return"&&";case t.bitwiseOR:return"|";case t.bitwiseXOR:return"^";case t.bitwiseAND:return"&";case t.equality:return"==/!=";case t.lessThan:return"<";case t.greaterThan:return">";case t.relationalOrEqual:return"<=/>=";case t.bitShiftL:return"<<";case t.bitShiftR:return">>/>>>";case t.plus:return"+";case t.minus:return"-";case t.modulo:return"%";case t.star:return"*";case t.slash:return"/";case t.exponent:return"**";case t.jsxName:return"jsxName";case t.jsxText:return"jsxText";case t.jsxEmptyText:return"jsxEmptyText";case t.jsxTagStart:return"jsxTagStart";case t.jsxTagEnd:return"jsxTagEnd";case t.typeParameterStart:return"typeParameterStart";case t.nonNullAssertion:return"nonNullAssertion";case t._break:return"break";case t._case:return"case";case t._catch:return"catch";case t._continue:return"continue";case t._debugger:return"debugger";case t._default:return"default";case t._do:return"do";case t._else:return"else";case t._finally:return"finally";case t._for:return"for";case t._function:return"function";case t._if:return"if";case t._return:return"return";case t._switch:return"switch";case t._throw:return"throw";case t._try:return"try";case t._var:return"var";case t._let:return"let";case t._const:return"const";case t._while:return"while";case t._with:return"with";case t._new:return"new";case t._this:return"this";case t._super:return"super";case t._class:return"class";case t._extends:return"extends";case t._export:return"export";case t._import:return"import";case t._yield:return"yield";case t._null:return"null";case t._true:return"true";case t._false:return"false";case t._in:return"in";case t._instanceof:return"instanceof";case t._typeof:return"typeof";case t._void:return"void";case t._delete:return"delete";case t._async:return"async";case t._get:return"get";case t._set:return"set";case t._declare:return"declare";case t._readonly:return"readonly";case t._abstract:return"abstract";case t._static:return"static";case t._public:return"public";case t._private:return"private";case t._protected:return"protected";case t._override:return"override";case t._as:return"as";case t._enum:return"enum";case t._type:return"type";case t._implements:return"implements";default:return""}}var te=class{constructor(n,r,o){this.startTokenIndex=n,this.endTokenIndex=r,this.isFunctionScope=o}},Kr=class{constructor(n,r,o,i,c,p,h,g,y,x,v,T,E){this.potentialArrowAt=n,this.noAnonFunctionType=r,this.inDisallowConditionalTypesContext=o,this.tokensLength=i,this.scopesLength=c,this.pos=p,this.type=h,this.contextualKeyword=g,this.start=y,this.end=x,this.isType=v,this.scopeDepth=T,this.error=E}},gt=class e{constructor(){e.prototype.__init.call(this),e.prototype.__init2.call(this),e.prototype.__init3.call(this),e.prototype.__init4.call(this),e.prototype.__init5.call(this),e.prototype.__init6.call(this),e.prototype.__init7.call(this),e.prototype.__init8.call(this),e.prototype.__init9.call(this),e.prototype.__init10.call(this),e.prototype.__init11.call(this),e.prototype.__init12.call(this),e.prototype.__init13.call(this)}__init(){this.potentialArrowAt=-1}__init2(){this.noAnonFunctionType=!1}__init3(){this.inDisallowConditionalTypesContext=!1}__init4(){this.tokens=[]}__init5(){this.scopes=[]}__init6(){this.pos=0}__init7(){this.type=t.eof}__init8(){this.contextualKeyword=l.NONE}__init9(){this.start=0}__init10(){this.end=0}__init11(){this.isType=!1}__init12(){this.scopeDepth=0}__init13(){this.error=null}snapshot(){return new Kr(this.potentialArrowAt,this.noAnonFunctionType,this.inDisallowConditionalTypesContext,this.tokens.length,this.scopes.length,this.pos,this.type,this.contextualKeyword,this.start,this.end,this.isType,this.scopeDepth,this.error)}restoreFromSnapshot(n){this.potentialArrowAt=n.potentialArrowAt,this.noAnonFunctionType=n.noAnonFunctionType,this.inDisallowConditionalTypesContext=n.inDisallowConditionalTypesContext,this.tokens.length=n.tokensLength,this.scopes.length=n.scopesLength,this.pos=n.pos,this.type=n.type,this.contextualKeyword=n.contextualKeyword,this.start=n.start,this.end=n.end,this.isType=n.isType,this.scopeDepth=n.scopeDepth,this.error=n.error}};var u;(function(e){e[e.backSpace=8]="backSpace";let r=10;e[e.lineFeed=r]="lineFeed";let o=9;e[e.tab=o]="tab";let i=13;e[e.carriageReturn=i]="carriageReturn";let c=14;e[e.shiftOut=c]="shiftOut";let p=32;e[e.space=p]="space";let h=33;e[e.exclamationMark=h]="exclamationMark";let g=34;e[e.quotationMark=g]="quotationMark";let y=35;e[e.numberSign=y]="numberSign";let x=36;e[e.dollarSign=x]="dollarSign";let v=37;e[e.percentSign=v]="percentSign";let T=38;e[e.ampersand=T]="ampersand";let E=39;e[e.apostrophe=E]="apostrophe";let O=40;e[e.leftParenthesis=O]="leftParenthesis";let q=41;e[e.rightParenthesis=q]="rightParenthesis";let G=42;e[e.asterisk=G]="asterisk";let N=43;e[e.plusSign=N]="plusSign";let J=44;e[e.comma=J]="comma";let ae=45;e[e.dash=ae]="dash";let ce=46;e[e.dot=ce]="dot";let je=47;e[e.slash=je]="slash";let Fe=48;e[e.digit0=Fe]="digit0";let Be=49;e[e.digit1=Be]="digit1";let qe=50;e[e.digit2=qe]="digit2";let $e=51;e[e.digit3=$e]="digit3";let Ue=52;e[e.digit4=Ue]="digit4";let He=53;e[e.digit5=He]="digit5";let Ve=54;e[e.digit6=Ve]="digit6";let We=55;e[e.digit7=We]="digit7";let ze=56;e[e.digit8=ze]="digit8";let Xe=57;e[e.digit9=Xe]="digit9";let Je=58;e[e.colon=Je]="colon";let Ge=59;e[e.semicolon=Ge]="semicolon";let Ke=60;e[e.lessThan=Ke]="lessThan";let Ye=61;e[e.equalsTo=Ye]="equalsTo";let Qe=62;e[e.greaterThan=Qe]="greaterThan";let Ze=63;e[e.questionMark=Ze]="questionMark";let et=64;e[e.atSign=et]="atSign";let tt=65;e[e.uppercaseA=tt]="uppercaseA";let nt=66;e[e.uppercaseB=nt]="uppercaseB";let mt=67;e[e.uppercaseC=mt]="uppercaseC";let Bn=68;e[e.uppercaseD=Bn]="uppercaseD";let qn=69;e[e.uppercaseE=qn]="uppercaseE";let $n=70;e[e.uppercaseF=$n]="uppercaseF";let Un=71;e[e.uppercaseG=Un]="uppercaseG";let Hn=72;e[e.uppercaseH=Hn]="uppercaseH";let Vn=73;e[e.uppercaseI=Vn]="uppercaseI";let Wn=74;e[e.uppercaseJ=Wn]="uppercaseJ";let zn=75;e[e.uppercaseK=zn]="uppercaseK";let Xn=76;e[e.uppercaseL=Xn]="uppercaseL";let Jn=77;e[e.uppercaseM=Jn]="uppercaseM";let Gn=78;e[e.uppercaseN=Gn]="uppercaseN";let Kn=79;e[e.uppercaseO=Kn]="uppercaseO";let Yn=80;e[e.uppercaseP=Yn]="uppercaseP";let Qn=81;e[e.uppercaseQ=Qn]="uppercaseQ";let Zn=82;e[e.uppercaseR=Zn]="uppercaseR";let er=83;e[e.uppercaseS=er]="uppercaseS";let tr=84;e[e.uppercaseT=tr]="uppercaseT";let nr=85;e[e.uppercaseU=nr]="uppercaseU";let rr=86;e[e.uppercaseV=rr]="uppercaseV";let sr=87;e[e.uppercaseW=sr]="uppercaseW";let or=88;e[e.uppercaseX=or]="uppercaseX";let ir=89;e[e.uppercaseY=ir]="uppercaseY";let ar=90;e[e.uppercaseZ=ar]="uppercaseZ";let cr=91;e[e.leftSquareBracket=cr]="leftSquareBracket";let lr=92;e[e.backslash=lr]="backslash";let ur=93;e[e.rightSquareBracket=ur]="rightSquareBracket";let pr=94;e[e.caret=pr]="caret";let fr=95;e[e.underscore=fr]="underscore";let hr=96;e[e.graveAccent=hr]="graveAccent";let dr=97;e[e.lowercaseA=dr]="lowercaseA";let mr=98;e[e.lowercaseB=mr]="lowercaseB";let gr=99;e[e.lowercaseC=gr]="lowercaseC";let kr=100;e[e.lowercaseD=kr]="lowercaseD";let yr=101;e[e.lowercaseE=yr]="lowercaseE";let xr=102;e[e.lowercaseF=xr]="lowercaseF";let br=103;e[e.lowercaseG=br]="lowercaseG";let _r=104;e[e.lowercaseH=_r]="lowercaseH";let wr=105;e[e.lowercaseI=wr]="lowercaseI";let vr=106;e[e.lowercaseJ=vr]="lowercaseJ";let Tr=107;e[e.lowercaseK=Tr]="lowercaseK";let Ir=108;e[e.lowercaseL=Ir]="lowercaseL";let Sr=109;e[e.lowercaseM=Sr]="lowercaseM";let Er=110;e[e.lowercaseN=Er]="lowercaseN";let Ar=111;e[e.lowercaseO=Ar]="lowercaseO";let Pr=112;e[e.lowercaseP=Pr]="lowercaseP";let Cr=113;e[e.lowercaseQ=Cr]="lowercaseQ";let Nr=114;e[e.lowercaseR=Nr]="lowercaseR";let Rr=115;e[e.lowercaseS=Rr]="lowercaseS";let Lr=116;e[e.lowercaseT=Lr]="lowercaseT";let Dr=117;e[e.lowercaseU=Dr]="lowercaseU";let Or=118;e[e.lowercaseV=Or]="lowercaseV";let Mr=119;e[e.lowercaseW=Mr]="lowercaseW";let jr=120;e[e.lowercaseX=jr]="lowercaseX";let Fr=121;e[e.lowercaseY=Fr]="lowercaseY";let Br=122;e[e.lowercaseZ=Br]="lowercaseZ";let qr=123;e[e.leftCurlyBrace=qr]="leftCurlyBrace";let $r=124;e[e.verticalBar=$r]="verticalBar";let Ur=125;e[e.rightCurlyBrace=Ur]="rightCurlyBrace";let Hr=126;e[e.tilde=Hr]="tilde";let Vr=160;e[e.nonBreakingSpace=Vr]="nonBreakingSpace";let Wr=5760;e[e.oghamSpaceMark=Wr]="oghamSpaceMark";let zr=8232;e[e.lineSeparator=zr]="lineSeparator";let Xr=8233;e[e.paragraphSeparator=Xr]="paragraphSeparator"})(u||(u={}));var rt,R,L,s,k,fo;function Ne(){return fo++}function ho(e){if("pos"in e){let n=ca(e.pos);e.message+=` (${n.line}:${n.column})`,e.loc=n}return e}var Yr=class{constructor(n,r){this.line=n,this.column=r}};function ca(e){let n=1,r=1;for(let o=0;o<e;o++)k.charCodeAt(o)===u.lineFeed?(n++,r=1):r++;return new Yr(n,r)}function mo(e,n,r,o){k=e,s=new gt,fo=1,rt=n,R=r,L=o}function b(e){return s.contextualKeyword===e}function st(e){let n=xe();return n.type===t.name&&n.contextualKeyword===e}function W(e){return s.contextualKeyword===e&&f(t.name)}function U(e){W(e)||S()}function Y(){return a(t.eof)||a(t.braceR)||K()}function K(){let e=s.tokens[s.tokens.length-1],n=e?e.end:0;for(let r=n;r<s.start;r++){let o=k.charCodeAt(r);if(o===u.lineFeed||o===u.carriageReturn||o===8232||o===8233)return!0}return!1}function tn(){let e=kt();for(let n=s.end;n<e;n++){let r=k.charCodeAt(n);if(r===u.lineFeed||r===u.carriageReturn||r===8232||r===8233)return!0}return!1}function le(){return f(t.semi)||Y()}function F(){le()||S('Unexpected token, expected ";"')}function d(e){f(e)||S(`Unexpected token, expected "${Gr(e)}"`)}function S(e="Unexpected token",n=s.start){if(s.error)return;let r=new SyntaxError(e);r.pos=n,s.error=r,s.pos=k.length,A(t.eof)}var Qr=[9,11,12,u.space,u.nonBreakingSpace,u.oghamSpaceMark,8192,8193,8194,8195,8196,8197,8198,8199,8200,8201,8202,8239,8287,12288,65279],Zr=/(?:\s|\/\/.*|\/\*[^]*?\*\/)*/g,es=new Uint8Array(65536);for(let e of Qr)es[e]=1;function la(e){if(e<48)return e===36;if(e<58)return!0;if(e<65)return!1;if(e<91)return!0;if(e<97)return e===95;if(e<123)return!0;if(e<128)return!1;throw new Error("Should not be called with non-ASCII char code.")}var Z=new Uint8Array(65536);for(let e=0;e<128;e++)Z[e]=la(e)?1:0;for(let e=128;e<65536;e++)Z[e]=1;for(let e of Qr)Z[e]=0;Z[8232]=0;Z[8233]=0;var be=Z.slice();for(let e=u.digit0;e<=u.digit9;e++)be[e]=0;var ts=new Int32Array([-1,27,783,918,1755,2376,2862,3483,-1,3699,-1,4617,4752,4833,5130,5508,5940,-1,6480,6939,7749,8181,8451,8613,-1,8829,-1,-1,-1,54,243,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,432,-1,-1,-1,675,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,81,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,108,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,135,-1,-1,-1,-1,-1,-1,-1,-1,-1,162,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,189,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,216,-1,-1,-1,-1,-1,-1,l._abstract<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,270,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,297,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,324,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,351,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,378,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,405,-1,-1,-1,-1,-1,-1,-1,-1,l._accessor<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._as<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,459,-1,-1,-1,-1,-1,594,-1,-1,-1,-1,-1,-1,486,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,513,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,540,-1,-1,-1,-1,-1,-1,l._assert<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,567,-1,-1,-1,-1,-1,-1,-1,l._asserts<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,621,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,648,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._async<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,702,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,729,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,756,-1,-1,-1,-1,-1,-1,l._await<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,810,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,837,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,864,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,891,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._break<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,945,-1,-1,-1,-1,-1,-1,1107,-1,-1,-1,1242,-1,-1,1350,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,972,1026,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,999,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._case<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1053,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1080,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._catch<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1134,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1161,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1188,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1215,-1,-1,-1,-1,-1,-1,-1,l._checks<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1269,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1296,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1323,-1,-1,-1,-1,-1,-1,-1,(t._class<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1377,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1404,1620,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1431,-1,-1,-1,-1,-1,-1,(t._const<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1458,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1485,-1,-1,-1,-1,-1,-1,-1,-1,1512,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1539,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1566,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1593,-1,-1,-1,-1,-1,-1,-1,-1,l._constructor<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1647,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1674,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1701,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1728,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._continue<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1782,-1,-1,-1,-1,-1,-1,-1,-1,-1,2349,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1809,1971,-1,-1,2106,-1,-1,-1,-1,-1,2241,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1836,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1863,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1890,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1917,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1944,-1,-1,-1,-1,-1,-1,-1,-1,(t._debugger<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,1998,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2025,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2052,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2079,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._declare<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2133,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2160,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2187,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2214,-1,-1,-1,-1,-1,-1,(t._default<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2268,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2295,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2322,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._delete<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._do<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2403,-1,2484,-1,-1,-1,-1,-1,-1,-1,-1,-1,2565,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2430,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2457,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._else<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2511,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2538,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._enum<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2592,-1,-1,-1,2727,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2619,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2646,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2673,-1,-1,-1,-1,-1,-1,(t._export<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2700,-1,-1,-1,-1,-1,-1,-1,l._exports<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2754,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2781,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2808,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2835,-1,-1,-1,-1,-1,-1,-1,(t._extends<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2889,-1,-1,-1,-1,-1,-1,-1,2997,-1,-1,-1,-1,-1,3159,-1,-1,3213,-1,-1,3294,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2916,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2943,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,2970,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._false<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3024,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3051,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3078,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3105,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3132,-1,(t._finally<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3186,-1,-1,-1,-1,-1,-1,-1,-1,(t._for<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3240,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3267,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._from<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3321,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3348,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3375,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3402,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3429,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3456,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._function<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3510,-1,-1,-1,-1,-1,-1,3564,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3537,-1,-1,-1,-1,-1,-1,l._get<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3591,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3618,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3645,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3672,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._global<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3726,-1,-1,-1,-1,-1,-1,3753,4077,-1,-1,-1,-1,4590,-1,-1,-1,-1,-1,-1,-1,(t._if<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3780,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3807,-1,-1,3996,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3834,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3861,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3888,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3915,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3942,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,3969,-1,-1,-1,-1,-1,-1,-1,l._implements<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4023,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4050,-1,-1,-1,-1,-1,-1,(t._import<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._in<<1)+1,-1,-1,-1,-1,-1,4104,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4185,4401,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4131,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4158,-1,-1,-1,-1,-1,-1,-1,-1,l._infer<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4212,-1,-1,-1,-1,-1,-1,-1,4239,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4266,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4293,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4320,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4347,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4374,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._instanceof<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4428,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4455,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4482,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4509,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4536,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4563,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._interface<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._is<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4644,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4671,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4698,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4725,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._keyof<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4779,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4806,-1,-1,-1,-1,-1,-1,(t._let<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4860,-1,-1,-1,-1,-1,4995,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4887,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4914,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4941,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,4968,-1,-1,-1,-1,-1,-1,-1,l._mixins<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5022,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5049,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5076,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5103,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._module<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5157,-1,-1,-1,5373,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5427,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5184,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5211,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5238,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5265,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5292,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5319,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5346,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._namespace<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5400,-1,-1,-1,(t._new<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5454,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5481,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._null<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5535,-1,-1,-1,-1,-1,-1,-1,-1,-1,5562,-1,-1,-1,-1,5697,5751,-1,-1,-1,-1,l._of<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5589,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5616,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5643,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5670,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._opaque<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5724,-1,-1,-1,-1,-1,-1,l._out<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5778,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5805,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5832,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5859,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5886,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5913,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._override<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5967,-1,-1,6345,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,5994,-1,-1,-1,-1,-1,6129,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6021,-1,-1,-1,-1,-1,6048,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6075,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6102,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._private<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6156,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6183,-1,-1,-1,-1,-1,-1,-1,-1,-1,6318,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6210,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6237,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6264,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6291,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._protected<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._proto<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6372,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6399,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6426,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6453,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._public<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6507,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6534,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6696,-1,-1,6831,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6561,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6588,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6615,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6642,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6669,-1,l._readonly<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6723,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6750,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6777,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6804,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._require<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6858,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6885,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6912,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._return<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6966,-1,-1,-1,7182,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7236,7371,-1,7479,-1,7614,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,6993,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7020,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7047,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7074,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7101,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7128,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7155,-1,-1,-1,-1,-1,-1,-1,l._satisfies<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7209,-1,-1,-1,-1,-1,-1,l._set<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7263,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7290,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7317,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7344,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._static<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7398,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7425,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7452,-1,-1,-1,-1,-1,-1,-1,-1,(t._super<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7506,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7533,-1,-1,-1,-1,-1,-1,-1,-1,-1,7560,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7587,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._switch<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7641,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7668,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7695,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7722,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._symbol<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7776,-1,-1,-1,-1,-1,-1,-1,-1,-1,7938,-1,-1,-1,-1,-1,-1,8046,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7803,-1,-1,-1,-1,-1,-1,-1,-1,7857,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7830,-1,-1,-1,-1,-1,-1,-1,(t._this<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7884,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7911,-1,-1,-1,(t._throw<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,7965,-1,-1,-1,8019,-1,-1,-1,-1,-1,-1,7992,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._true<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._try<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8073,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8100,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._type<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8127,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8154,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._typeof<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8208,-1,-1,-1,-1,8343,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8235,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8262,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8289,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8316,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._unique<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8370,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8397,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8424,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,l._using<<1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8478,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8532,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8505,-1,-1,-1,-1,-1,-1,-1,-1,(t._var<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8559,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8586,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._void<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8640,8748,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8667,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8694,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8721,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._while<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8775,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8802,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._with<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8856,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8883,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8910,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,8937,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,(t._yield<<1)+1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1]);function ns(){let e=0,n=0,r=s.pos;for(;r<k.length&&(n=k.charCodeAt(r),!(n<u.lowercaseA||n>u.lowercaseZ));){let i=ts[e+(n-u.lowercaseA)+1];if(i===-1)break;e=i,r++}let o=ts[e];if(o>-1&&!Z[n]){s.pos=r,o&1?A(o>>>1):A(t.name,o>>>1);return}for(;r<k.length;){let i=k.charCodeAt(r);if(Z[i])r++;else if(i===u.backslash){if(r+=2,k.charCodeAt(r)===u.leftCurlyBrace){for(;r<k.length&&k.charCodeAt(r)!==u.rightCurlyBrace;)r++;r++}}else if(i===u.atSign&&k.charCodeAt(r+1)===u.atSign)r+=2;else break}s.pos=r,A(t.name)}var _;(function(e){e[e.Access=0]="Access";let r=1;e[e.ExportAccess=r]="ExportAccess";let o=r+1;e[e.TopLevelDeclaration=o]="TopLevelDeclaration";let i=o+1;e[e.FunctionScopedDeclaration=i]="FunctionScopedDeclaration";let c=i+1;e[e.BlockScopedDeclaration=c]="BlockScopedDeclaration";let p=c+1;e[e.ObjectShorthandTopLevelDeclaration=p]="ObjectShorthandTopLevelDeclaration";let h=p+1;e[e.ObjectShorthandFunctionScopedDeclaration=h]="ObjectShorthandFunctionScopedDeclaration";let g=h+1;e[e.ObjectShorthandBlockScopedDeclaration=g]="ObjectShorthandBlockScopedDeclaration";let y=g+1;e[e.ObjectShorthand=y]="ObjectShorthand";let x=y+1;e[e.ImportDeclaration=x]="ImportDeclaration";let v=x+1;e[e.ObjectKey=v]="ObjectKey";let T=v+1;e[e.ImportAccess=T]="ImportAccess"})(_||(_={}));var se;(function(e){e[e.NoChildren=0]="NoChildren";let r=1;e[e.OneChild=r]="OneChild";let o=r+1;e[e.StaticChildren=o]="StaticChildren";let i=o+1;e[e.KeyAfterPropSpread=i]="KeyAfterPropSpread"})(se||(se={}));function nn(e){let n=e.identifierRole;return n===_.TopLevelDeclaration||n===_.FunctionScopedDeclaration||n===_.BlockScopedDeclaration||n===_.ObjectShorthandTopLevelDeclaration||n===_.ObjectShorthandFunctionScopedDeclaration||n===_.ObjectShorthandBlockScopedDeclaration}function go(e){let n=e.identifierRole;return n===_.FunctionScopedDeclaration||n===_.BlockScopedDeclaration||n===_.ObjectShorthandFunctionScopedDeclaration||n===_.ObjectShorthandBlockScopedDeclaration}function rn(e){let n=e.identifierRole;return n===_.TopLevelDeclaration||n===_.ObjectShorthandTopLevelDeclaration||n===_.ImportDeclaration}function ko(e){let n=e.identifierRole;return n===_.TopLevelDeclaration||n===_.BlockScopedDeclaration||n===_.ObjectShorthandTopLevelDeclaration||n===_.ObjectShorthandBlockScopedDeclaration}function yo(e){let n=e.identifierRole;return n===_.FunctionScopedDeclaration||n===_.ObjectShorthandFunctionScopedDeclaration}function xo(e){return e.identifierRole===_.ObjectShorthandTopLevelDeclaration||e.identifierRole===_.ObjectShorthandBlockScopedDeclaration||e.identifierRole===_.ObjectShorthandFunctionScopedDeclaration}var Re=class{constructor(){this.type=s.type,this.contextualKeyword=s.contextualKeyword,this.start=s.start,this.end=s.end,this.scopeDepth=s.scopeDepth,this.isType=s.isType,this.identifierRole=null,this.jsxRole=null,this.shadowsGlobal=!1,this.isAsyncOperation=!1,this.contextId=null,this.rhsEndIndex=null,this.isExpression=!1,this.numNullishCoalesceStarts=0,this.numNullishCoalesceEnds=0,this.isOptionalChainStart=!1,this.isOptionalChainEnd=!1,this.subscriptStartIndex=null,this.nullishStartIndex=null}};function m(){s.tokens.push(new Re),as()}function me(){s.tokens.push(new Re),s.start=s.pos,Ta()}function bo(){s.type===t.assign&&--s.pos,_a()}function C(e){for(let r=s.tokens.length-e;r<s.tokens.length;r++)s.tokens[r].isType=!0;let n=s.isType;return s.isType=!0,n}function P(e){s.isType=e}function f(e){return a(e)?(m(),!0):!1}function sn(e){let n=s.isType;s.isType=!0,f(e),s.isType=n}function a(e){return s.type===e}function B(){let e=s.snapshot();m();let n=s.type;return s.restoreFromSnapshot(e),n}var ss=class{constructor(n,r){this.type=n,this.contextualKeyword=r}};function xe(){let e=s.snapshot();m();let n=s.type,r=s.contextualKeyword;return s.restoreFromSnapshot(e),new ss(n,r)}function kt(){return os(s.pos)}function os(e){Zr.lastIndex=e;let n=Zr.exec(k);return e+n[0].length}function is(){return k.charCodeAt(kt())}function as(){if(ls(),s.start=s.pos,s.pos>=k.length){let e=s.tokens;e.length>=2&&e[e.length-1].start>=k.length&&e[e.length-2].start>=k.length&&S("Unexpectedly reached the end of input."),A(t.eof);return}ua(k.charCodeAt(s.pos))}function ua(e){be[e]||e===u.backslash||e===u.atSign&&k.charCodeAt(s.pos+1)===u.atSign?ns():us(e)}function pa(){for(;k.charCodeAt(s.pos)!==u.asterisk||k.charCodeAt(s.pos+1)!==u.slash;)if(s.pos++,s.pos>k.length){S("Unterminated comment",s.pos-2);return}s.pos+=2}function cs(e){let n=k.charCodeAt(s.pos+=e);if(s.pos<k.length)for(;n!==u.lineFeed&&n!==u.carriageReturn&&n!==u.lineSeparator&&n!==u.paragraphSeparator&&++s.pos<k.length;)n=k.charCodeAt(s.pos)}function ls(){for(;s.pos<k.length;){let e=k.charCodeAt(s.pos);switch(e){case u.carriageReturn:k.charCodeAt(s.pos+1)===u.lineFeed&&++s.pos;case u.lineFeed:case u.lineSeparator:case u.paragraphSeparator:++s.pos;break;case u.slash:switch(k.charCodeAt(s.pos+1)){case u.asterisk:s.pos+=2,pa();break;case u.slash:cs(2);break;default:return}break;default:if(es[e])++s.pos;else return}}}function A(e,n=l.NONE){s.end=s.pos,s.type=e,s.contextualKeyword=n}function fa(){let e=k.charCodeAt(s.pos+1);if(e>=u.digit0&&e<=u.digit9){wo(!0);return}e===u.dot&&k.charCodeAt(s.pos+2)===u.dot?(s.pos+=3,A(t.ellipsis)):(++s.pos,A(t.dot))}function ha(){k.charCodeAt(s.pos+1)===u.equalsTo?M(t.assign,2):M(t.slash,1)}function da(e){let n=e===u.asterisk?t.star:t.modulo,r=1,o=k.charCodeAt(s.pos+1);e===u.asterisk&&o===u.asterisk&&(r++,o=k.charCodeAt(s.pos+2),n=t.exponent),o===u.equalsTo&&k.charCodeAt(s.pos+2)!==u.greaterThan&&(r++,n=t.assign),M(n,r)}function ma(e){let n=k.charCodeAt(s.pos+1);if(n===e){k.charCodeAt(s.pos+2)===u.equalsTo?M(t.assign,3):M(e===u.verticalBar?t.logicalOR:t.logicalAND,2);return}if(e===u.verticalBar){if(n===u.greaterThan){M(t.pipeline,2);return}else if(n===u.rightCurlyBrace&&L){M(t.braceBarR,2);return}}if(n===u.equalsTo){M(t.assign,2);return}M(e===u.verticalBar?t.bitwiseOR:t.bitwiseAND,1)}function ga(){k.charCodeAt(s.pos+1)===u.equalsTo?M(t.assign,2):M(t.bitwiseXOR,1)}function ka(e){let n=k.charCodeAt(s.pos+1);if(n===e){M(t.preIncDec,2);return}n===u.equalsTo?M(t.assign,2):e===u.plusSign?M(t.plus,1):M(t.minus,1)}function ya(){let e=k.charCodeAt(s.pos+1);if(e===u.lessThan){if(k.charCodeAt(s.pos+2)===u.equalsTo){M(t.assign,3);return}s.isType?M(t.lessThan,1):M(t.bitShiftL,2);return}e===u.equalsTo?M(t.relationalOrEqual,2):M(t.lessThan,1)}function _o(){if(s.isType){M(t.greaterThan,1);return}let e=k.charCodeAt(s.pos+1);if(e===u.greaterThan){let n=k.charCodeAt(s.pos+2)===u.greaterThan?3:2;if(k.charCodeAt(s.pos+n)===u.equalsTo){M(t.assign,n+1);return}M(t.bitShiftR,n);return}e===u.equalsTo?M(t.relationalOrEqual,2):M(t.greaterThan,1)}function on(){s.type===t.greaterThan&&(s.pos-=1,_o())}function xa(e){let n=k.charCodeAt(s.pos+1);if(n===u.equalsTo){M(t.equality,k.charCodeAt(s.pos+2)===u.equalsTo?3:2);return}if(e===u.equalsTo&&n===u.greaterThan){s.pos+=2,A(t.arrow);return}M(e===u.equalsTo?t.eq:t.bang,1)}function ba(){let e=k.charCodeAt(s.pos+1),n=k.charCodeAt(s.pos+2);e===u.questionMark&&!(L&&s.isType)?n===u.equalsTo?M(t.assign,3):M(t.nullishCoalescing,2):e===u.dot&&!(n>=u.digit0&&n<=u.digit9)?(s.pos+=2,A(t.questionDot)):(++s.pos,A(t.question))}function us(e){switch(e){case u.numberSign:++s.pos,A(t.hash);return;case u.dot:fa();return;case u.leftParenthesis:++s.pos,A(t.parenL);return;case u.rightParenthesis:++s.pos,A(t.parenR);return;case u.semicolon:++s.pos,A(t.semi);return;case u.comma:++s.pos,A(t.comma);return;case u.leftSquareBracket:++s.pos,A(t.bracketL);return;case u.rightSquareBracket:++s.pos,A(t.bracketR);return;case u.leftCurlyBrace:L&&k.charCodeAt(s.pos+1)===u.verticalBar?M(t.braceBarL,2):(++s.pos,A(t.braceL));return;case u.rightCurlyBrace:++s.pos,A(t.braceR);return;case u.colon:k.charCodeAt(s.pos+1)===u.colon?M(t.doubleColon,2):(++s.pos,A(t.colon));return;case u.questionMark:ba();return;case u.atSign:++s.pos,A(t.at);return;case u.graveAccent:++s.pos,A(t.backQuote);return;case u.digit0:{let n=k.charCodeAt(s.pos+1);if(n===u.lowercaseX||n===u.uppercaseX||n===u.lowercaseO||n===u.uppercaseO||n===u.lowercaseB||n===u.uppercaseB){wa();return}}case u.digit1:case u.digit2:case u.digit3:case u.digit4:case u.digit5:case u.digit6:case u.digit7:case u.digit8:case u.digit9:wo(!1);return;case u.quotationMark:case u.apostrophe:va(e);return;case u.slash:ha();return;case u.percentSign:case u.asterisk:da(e);return;case u.verticalBar:case u.ampersand:ma(e);return;case u.caret:ga();return;case u.plusSign:case u.dash:ka(e);return;case u.lessThan:ya();return;case u.greaterThan:_o();return;case u.equalsTo:case u.exclamationMark:xa(e);return;case u.tilde:M(t.tilde,1);return;default:break}S(`Unexpected character '${String.fromCharCode(e)}'`,s.pos)}function M(e,n){s.pos+=n,A(e)}function _a(){let e=s.pos,n=!1,r=!1;for(;;){if(s.pos>=k.length){S("Unterminated regular expression",e);return}let o=k.charCodeAt(s.pos);if(n)n=!1;else{if(o===u.leftSquareBracket)r=!0;else if(o===u.rightSquareBracket&&r)r=!1;else if(o===u.slash&&!r)break;n=o===u.backslash}++s.pos}++s.pos,Ia(),A(t.regexp)}function rs(){for(;;){let e=k.charCodeAt(s.pos);if(e>=u.digit0&&e<=u.digit9||e===u.underscore)s.pos++;else break}}function wa(){for(s.pos+=2;;){let n=k.charCodeAt(s.pos);if(n>=u.digit0&&n<=u.digit9||n>=u.lowercaseA&&n<=u.lowercaseF||n>=u.uppercaseA&&n<=u.uppercaseF||n===u.underscore)s.pos++;else break}k.charCodeAt(s.pos)===u.lowercaseN?(++s.pos,A(t.bigint)):A(t.num)}function wo(e){let n=!1,r=!1;e||rs();let o=k.charCodeAt(s.pos);if(o===u.dot&&(++s.pos,rs(),o=k.charCodeAt(s.pos)),(o===u.uppercaseE||o===u.lowercaseE)&&(o=k.charCodeAt(++s.pos),(o===u.plusSign||o===u.dash)&&++s.pos,rs(),o=k.charCodeAt(s.pos)),o===u.lowercaseN?(++s.pos,n=!0):o===u.lowercaseM&&(++s.pos,r=!0),n){A(t.bigint);return}if(r){A(t.decimal);return}A(t.num)}function va(e){for(s.pos++;;){if(s.pos>=k.length){S("Unterminated string constant");return}let n=k.charCodeAt(s.pos);if(n===u.backslash)s.pos++;else if(n===e)break;s.pos++}s.pos++,A(t.string)}function Ta(){for(;;){if(s.pos>=k.length){S("Unterminated template");return}let e=k.charCodeAt(s.pos);if(e===u.graveAccent||e===u.dollarSign&&k.charCodeAt(s.pos+1)===u.leftCurlyBrace){if(s.pos===s.start&&a(t.template))if(e===u.dollarSign){s.pos+=2,A(t.dollarBraceL);return}else{++s.pos,A(t.backQuote);return}A(t.template);return}e===u.backslash&&s.pos++,s.pos++}}function Ia(){for(;s.pos<k.length;){let e=k.charCodeAt(s.pos);if(Z[e])s.pos++;else if(e===u.backslash){if(s.pos+=2,k.charCodeAt(s.pos)===u.leftCurlyBrace){for(;s.pos<k.length&&k.charCodeAt(s.pos)!==u.rightCurlyBrace;)s.pos++;s.pos++}}else break}}function ge(e,n=e.currentIndex()){let r=n+1;if(an(e,r)){let o=e.identifierNameAtIndex(n);return{isType:!1,leftName:o,rightName:o,endIndex:r}}if(r++,an(e,r))return{isType:!0,leftName:null,rightName:null,endIndex:r};if(r++,an(e,r))return{isType:!1,leftName:e.identifierNameAtIndex(n),rightName:e.identifierNameAtIndex(n+2),endIndex:r};if(r++,an(e,r))return{isType:!0,leftName:null,rightName:null,endIndex:r};throw new Error(`Unexpected import/export specifier at ${n}`)}function an(e,n){let r=e.tokens[n];return r.type===t.braceR||r.type===t.comma}var vo=new Map([["quot",'"'],["amp","&"],["apos","'"],["lt","<"],["gt",">"],["nbsp","\xA0"],["iexcl","\xA1"],["cent","\xA2"],["pound","\xA3"],["curren","\xA4"],["yen","\xA5"],["brvbar","\xA6"],["sect","\xA7"],["uml","\xA8"],["copy","\xA9"],["ordf","\xAA"],["laquo","\xAB"],["not","\xAC"],["shy","\xAD"],["reg","\xAE"],["macr","\xAF"],["deg","\xB0"],["plusmn","\xB1"],["sup2","\xB2"],["sup3","\xB3"],["acute","\xB4"],["micro","\xB5"],["para","\xB6"],["middot","\xB7"],["cedil","\xB8"],["sup1","\xB9"],["ordm","\xBA"],["raquo","\xBB"],["frac14","\xBC"],["frac12","\xBD"],["frac34","\xBE"],["iquest","\xBF"],["Agrave","\xC0"],["Aacute","\xC1"],["Acirc","\xC2"],["Atilde","\xC3"],["Auml","\xC4"],["Aring","\xC5"],["AElig","\xC6"],["Ccedil","\xC7"],["Egrave","\xC8"],["Eacute","\xC9"],["Ecirc","\xCA"],["Euml","\xCB"],["Igrave","\xCC"],["Iacute","\xCD"],["Icirc","\xCE"],["Iuml","\xCF"],["ETH","\xD0"],["Ntilde","\xD1"],["Ograve","\xD2"],["Oacute","\xD3"],["Ocirc","\xD4"],["Otilde","\xD5"],["Ouml","\xD6"],["times","\xD7"],["Oslash","\xD8"],["Ugrave","\xD9"],["Uacute","\xDA"],["Ucirc","\xDB"],["Uuml","\xDC"],["Yacute","\xDD"],["THORN","\xDE"],["szlig","\xDF"],["agrave","\xE0"],["aacute","\xE1"],["acirc","\xE2"],["atilde","\xE3"],["auml","\xE4"],["aring","\xE5"],["aelig","\xE6"],["ccedil","\xE7"],["egrave","\xE8"],["eacute","\xE9"],["ecirc","\xEA"],["euml","\xEB"],["igrave","\xEC"],["iacute","\xED"],["icirc","\xEE"],["iuml","\xEF"],["eth","\xF0"],["ntilde","\xF1"],["ograve","\xF2"],["oacute","\xF3"],["ocirc","\xF4"],["otilde","\xF5"],["ouml","\xF6"],["divide","\xF7"],["oslash","\xF8"],["ugrave","\xF9"],["uacute","\xFA"],["ucirc","\xFB"],["uuml","\xFC"],["yacute","\xFD"],["thorn","\xFE"],["yuml","\xFF"],["OElig","\u0152"],["oelig","\u0153"],["Scaron","\u0160"],["scaron","\u0161"],["Yuml","\u0178"],["fnof","\u0192"],["circ","\u02C6"],["tilde","\u02DC"],["Alpha","\u0391"],["Beta","\u0392"],["Gamma","\u0393"],["Delta","\u0394"],["Epsilon","\u0395"],["Zeta","\u0396"],["Eta","\u0397"],["Theta","\u0398"],["Iota","\u0399"],["Kappa","\u039A"],["Lambda","\u039B"],["Mu","\u039C"],["Nu","\u039D"],["Xi","\u039E"],["Omicron","\u039F"],["Pi","\u03A0"],["Rho","\u03A1"],["Sigma","\u03A3"],["Tau","\u03A4"],["Upsilon","\u03A5"],["Phi","\u03A6"],["Chi","\u03A7"],["Psi","\u03A8"],["Omega","\u03A9"],["alpha","\u03B1"],["beta","\u03B2"],["gamma","\u03B3"],["delta","\u03B4"],["epsilon","\u03B5"],["zeta","\u03B6"],["eta","\u03B7"],["theta","\u03B8"],["iota","\u03B9"],["kappa","\u03BA"],["lambda","\u03BB"],["mu","\u03BC"],["nu","\u03BD"],["xi","\u03BE"],["omicron","\u03BF"],["pi","\u03C0"],["rho","\u03C1"],["sigmaf","\u03C2"],["sigma","\u03C3"],["tau","\u03C4"],["upsilon","\u03C5"],["phi","\u03C6"],["chi","\u03C7"],["psi","\u03C8"],["omega","\u03C9"],["thetasym","\u03D1"],["upsih","\u03D2"],["piv","\u03D6"],["ensp","\u2002"],["emsp","\u2003"],["thinsp","\u2009"],["zwnj","\u200C"],["zwj","\u200D"],["lrm","\u200E"],["rlm","\u200F"],["ndash","\u2013"],["mdash","\u2014"],["lsquo","\u2018"],["rsquo","\u2019"],["sbquo","\u201A"],["ldquo","\u201C"],["rdquo","\u201D"],["bdquo","\u201E"],["dagger","\u2020"],["Dagger","\u2021"],["bull","\u2022"],["hellip","\u2026"],["permil","\u2030"],["prime","\u2032"],["Prime","\u2033"],["lsaquo","\u2039"],["rsaquo","\u203A"],["oline","\u203E"],["frasl","\u2044"],["euro","\u20AC"],["image","\u2111"],["weierp","\u2118"],["real","\u211C"],["trade","\u2122"],["alefsym","\u2135"],["larr","\u2190"],["uarr","\u2191"],["rarr","\u2192"],["darr","\u2193"],["harr","\u2194"],["crarr","\u21B5"],["lArr","\u21D0"],["uArr","\u21D1"],["rArr","\u21D2"],["dArr","\u21D3"],["hArr","\u21D4"],["forall","\u2200"],["part","\u2202"],["exist","\u2203"],["empty","\u2205"],["nabla","\u2207"],["isin","\u2208"],["notin","\u2209"],["ni","\u220B"],["prod","\u220F"],["sum","\u2211"],["minus","\u2212"],["lowast","\u2217"],["radic","\u221A"],["prop","\u221D"],["infin","\u221E"],["ang","\u2220"],["and","\u2227"],["or","\u2228"],["cap","\u2229"],["cup","\u222A"],["int","\u222B"],["there4","\u2234"],["sim","\u223C"],["cong","\u2245"],["asymp","\u2248"],["ne","\u2260"],["equiv","\u2261"],["le","\u2264"],["ge","\u2265"],["sub","\u2282"],["sup","\u2283"],["nsub","\u2284"],["sube","\u2286"],["supe","\u2287"],["oplus","\u2295"],["otimes","\u2297"],["perp","\u22A5"],["sdot","\u22C5"],["lceil","\u2308"],["rceil","\u2309"],["lfloor","\u230A"],["rfloor","\u230B"],["lang","\u2329"],["rang","\u232A"],["loz","\u25CA"],["spades","\u2660"],["clubs","\u2663"],["hearts","\u2665"],["diams","\u2666"]]);function yt(e){let[n,r]=To(e.jsxPragma||"React.createElement"),[o,i]=To(e.jsxFragmentPragma||"React.Fragment");return{base:n,suffix:r,fragmentBase:o,fragmentSuffix:i}}function To(e){let n=e.indexOf(".");return n===-1&&(n=e.length),[e.slice(0,n),e.slice(n)]}var H=class{getPrefixCode(){return""}getHoistedCode(){return""}getSuffixCode(){return""}};var xt=class e extends H{__init(){this.lastLineNumber=1}__init2(){this.lastIndex=0}__init3(){this.filenameVarName=null}__init4(){this.esmAutomaticImportNameResolutions={}}__init5(){this.cjsAutomaticModuleNameResolutions={}}constructor(n,r,o,i,c){super(),this.rootTransformer=n,this.tokens=r,this.importProcessor=o,this.nameManager=i,this.options=c,e.prototype.__init.call(this),e.prototype.__init2.call(this),e.prototype.__init3.call(this),e.prototype.__init4.call(this),e.prototype.__init5.call(this),this.jsxPragmaInfo=yt(c),this.isAutomaticRuntime=c.jsxRuntime==="automatic",this.jsxImportSource=c.jsxImportSource||"react"}process(){return this.tokens.matches1(t.jsxTagStart)?(this.processJSXTag(),!0):!1}getPrefixCode(){let n="";if(this.filenameVarName&&(n+=`const ${this.filenameVarName} = ${JSON.stringify(this.options.filePath||"")};`),this.isAutomaticRuntime)if(this.importProcessor)for(let[r,o]of Object.entries(this.cjsAutomaticModuleNameResolutions))n+=`var ${o} = require("${r}");`;else{let{createElement:r,...o}=this.esmAutomaticImportNameResolutions;r&&(n+=`import {createElement as ${r}} from "${this.jsxImportSource}";`);let i=Object.entries(o).map(([c,p])=>`${c} as ${p}`).join(", ");if(i){let c=this.jsxImportSource+(this.options.production?"/jsx-runtime":"/jsx-dev-runtime");n+=`import {${i}} from "${c}";`}}return n}processJSXTag(){let{jsxRole:n,start:r}=this.tokens.currentToken(),o=this.options.production?null:this.getElementLocationCode(r);this.isAutomaticRuntime&&n!==se.KeyAfterPropSpread?this.transformTagToJSXFunc(o,n):this.transformTagToCreateElement(o)}getElementLocationCode(n){return`lineNumber: ${this.getLineNumberForIndex(n)}`}getLineNumberForIndex(n){let r=this.tokens.code;for(;this.lastIndex<n&&this.lastIndex<r.length;)r[this.lastIndex]===`
`&&this.lastLineNumber++,this.lastIndex++;return this.lastLineNumber}transformTagToJSXFunc(n,r){let o=r===se.StaticChildren;this.tokens.replaceToken(this.getJSXFuncInvocationCode(o));let i=null;if(this.tokens.matches1(t.jsxTagEnd))this.tokens.replaceToken(`${this.getFragmentCode()}, {`),this.processAutomaticChildrenAndEndProps(r);else{if(this.processTagIntro(),this.tokens.appendCode(", {"),i=this.processProps(!0),this.tokens.matches2(t.slash,t.jsxTagEnd))this.tokens.appendCode("}");else if(this.tokens.matches1(t.jsxTagEnd))this.tokens.removeToken(),this.processAutomaticChildrenAndEndProps(r);else throw new Error("Expected either /> or > at the end of the tag.");i&&this.tokens.appendCode(`, ${i}`)}for(this.options.production||(i===null&&this.tokens.appendCode(", void 0"),this.tokens.appendCode(`, ${o}, ${this.getDevSource(n)}, this`)),this.tokens.removeInitialToken();!this.tokens.matches1(t.jsxTagEnd);)this.tokens.removeToken();this.tokens.replaceToken(")")}transformTagToCreateElement(n){if(this.tokens.replaceToken(this.getCreateElementInvocationCode()),this.tokens.matches1(t.jsxTagEnd))this.tokens.replaceToken(`${this.getFragmentCode()}, null`),this.processChildren(!0);else if(this.processTagIntro(),this.processPropsObjectWithDevInfo(n),!this.tokens.matches2(t.slash,t.jsxTagEnd))if(this.tokens.matches1(t.jsxTagEnd))this.tokens.removeToken(),this.processChildren(!0);else throw new Error("Expected either /> or > at the end of the tag.");for(this.tokens.removeInitialToken();!this.tokens.matches1(t.jsxTagEnd);)this.tokens.removeToken();this.tokens.replaceToken(")")}getJSXFuncInvocationCode(n){return this.options.production?n?this.claimAutoImportedFuncInvocation("jsxs","/jsx-runtime"):this.claimAutoImportedFuncInvocation("jsx","/jsx-runtime"):this.claimAutoImportedFuncInvocation("jsxDEV","/jsx-dev-runtime")}getCreateElementInvocationCode(){if(this.isAutomaticRuntime)return this.claimAutoImportedFuncInvocation("createElement","");{let{jsxPragmaInfo:n}=this;return`${this.importProcessor&&this.importProcessor.getIdentifierReplacement(n.base)||n.base}${n.suffix}(`}}getFragmentCode(){if(this.isAutomaticRuntime)return this.claimAutoImportedName("Fragment",this.options.production?"/jsx-runtime":"/jsx-dev-runtime");{let{jsxPragmaInfo:n}=this;return(this.importProcessor&&this.importProcessor.getIdentifierReplacement(n.fragmentBase)||n.fragmentBase)+n.fragmentSuffix}}claimAutoImportedFuncInvocation(n,r){let o=this.claimAutoImportedName(n,r);return this.importProcessor?`${o}.call(void 0, `:`${o}(`}claimAutoImportedName(n,r){if(this.importProcessor){let o=this.jsxImportSource+r;return this.cjsAutomaticModuleNameResolutions[o]||(this.cjsAutomaticModuleNameResolutions[o]=this.importProcessor.getFreeIdentifierForPath(o)),`${this.cjsAutomaticModuleNameResolutions[o]}.${n}`}else return this.esmAutomaticImportNameResolutions[n]||(this.esmAutomaticImportNameResolutions[n]=this.nameManager.claimFreeName(`_${n}`)),this.esmAutomaticImportNameResolutions[n]}processTagIntro(){let n=this.tokens.currentIndex()+1;for(;this.tokens.tokens[n].isType||!this.tokens.matches2AtIndex(n-1,t.jsxName,t.jsxName)&&!this.tokens.matches2AtIndex(n-1,t.greaterThan,t.jsxName)&&!this.tokens.matches1AtIndex(n,t.braceL)&&!this.tokens.matches1AtIndex(n,t.jsxTagEnd)&&!this.tokens.matches2AtIndex(n,t.slash,t.jsxTagEnd);)n++;if(n===this.tokens.currentIndex()+1){let r=this.tokens.identifierName();ps(r)&&this.tokens.replaceToken(`'${r}'`)}for(;this.tokens.currentIndex()<n;)this.rootTransformer.processToken()}processPropsObjectWithDevInfo(n){let r=this.options.production?"":`__self: this, __source: ${this.getDevSource(n)}`;if(!this.tokens.matches1(t.jsxName)&&!this.tokens.matches1(t.braceL)){r?this.tokens.appendCode(`, {${r}}`):this.tokens.appendCode(", null");return}this.tokens.appendCode(", {"),this.processProps(!1),r?this.tokens.appendCode(` ${r}}`):this.tokens.appendCode("}")}processProps(n){let r=null;for(;;){if(this.tokens.matches2(t.jsxName,t.eq)){let o=this.tokens.identifierName();if(n&&o==="key"){r!==null&&this.tokens.appendCode(r.replace(/[^\n]/g,"")),this.tokens.removeToken(),this.tokens.removeToken();let i=this.tokens.snapshot();this.processPropValue(),r=this.tokens.dangerouslyGetAndRemoveCodeSinceSnapshot(i);continue}else this.processPropName(o),this.tokens.replaceToken(": "),this.processPropValue()}else if(this.tokens.matches1(t.jsxName)){let o=this.tokens.identifierName();this.processPropName(o),this.tokens.appendCode(": true")}else if(this.tokens.matches1(t.braceL))this.tokens.replaceToken(""),this.rootTransformer.processBalancedCode(),this.tokens.replaceToken("");else break;this.tokens.appendCode(",")}return r}processPropName(n){n.includes("-")?this.tokens.replaceToken(`'${n}'`):this.tokens.copyToken()}processPropValue(){this.tokens.matches1(t.braceL)?(this.tokens.replaceToken(""),this.rootTransformer.processBalancedCode(),this.tokens.replaceToken("")):this.tokens.matches1(t.jsxTagStart)?this.processJSXTag():this.processStringPropValue()}processStringPropValue(){let n=this.tokens.currentToken(),r=this.tokens.code.slice(n.start+1,n.end-1),o=Io(r),i=Ea(r);this.tokens.replaceToken(i+o)}processAutomaticChildrenAndEndProps(n){n===se.StaticChildren?(this.tokens.appendCode(" children: ["),this.processChildren(!1),this.tokens.appendCode("]}")):(n===se.OneChild&&this.tokens.appendCode(" children: "),this.processChildren(!1),this.tokens.appendCode("}"))}processChildren(n){let r=n;for(;;){if(this.tokens.matches2(t.jsxTagStart,t.slash))return;let o=!1;if(this.tokens.matches1(t.braceL))this.tokens.matches2(t.braceL,t.braceR)?(this.tokens.replaceToken(""),this.tokens.replaceToken("")):(this.tokens.replaceToken(r?", ":""),this.rootTransformer.processBalancedCode(),this.tokens.replaceToken(""),o=!0);else if(this.tokens.matches1(t.jsxTagStart))this.tokens.appendCode(r?", ":""),this.processJSXTag(),o=!0;else if(this.tokens.matches1(t.jsxText)||this.tokens.matches1(t.jsxEmptyText))o=this.processChildTextElement(r);else throw new Error("Unexpected token when processing JSX children.");o&&(r=!0)}}processChildTextElement(n){let r=this.tokens.currentToken(),o=this.tokens.code.slice(r.start,r.end),i=Io(o),c=Sa(o);return c==='""'?(this.tokens.replaceToken(i),!1):(this.tokens.replaceToken(`${n?", ":""}${c}${i}`),!0)}getDevSource(n){return`{fileName: ${this.getFilenameVarName()}, ${n}}`}getFilenameVarName(){return this.filenameVarName||(this.filenameVarName=this.nameManager.claimFreeName("_jsxFileName")),this.filenameVarName}};function ps(e){let n=e.charCodeAt(0);return n>=u.lowercaseA&&n<=u.lowercaseZ}function Sa(e){let n="",r="",o=!1,i=!1;for(let c=0;c<e.length;c++){let p=e[c];if(p===" "||p==="	"||p==="\r")o||(r+=p);else if(p===`
`)r="",o=!0;else{if(i&&o&&(n+=" "),n+=r,r="",p==="&"){let{entity:h,newI:g}=So(e,c+1);c=g-1,n+=h}else n+=p;i=!0,o=!1}}return o||(n+=r),JSON.stringify(n)}function Io(e){let n=0,r=0;for(let o of e)o===`
`?(n++,r=0):o===" "&&r++;return`
`.repeat(n)+" ".repeat(r)}function Ea(e){let n="";for(let r=0;r<e.length;r++){let o=e[r];if(o===`
`)if(/\s/.test(e[r+1]))for(n+=" ";r<e.length&&/\s/.test(e[r+1]);)r++;else n+=`
`;else if(o==="&"){let{entity:i,newI:c}=So(e,r+1);n+=i,r=c-1}else n+=o}return JSON.stringify(n)}function So(e,n){let r="",o=0,i,c=n;if(e[c]==="#"){let p=10;c++;let h;if(e[c]==="x")for(p=16,c++,h=c;c<e.length&&Pa(e.charCodeAt(c));)c++;else for(h=c;c<e.length&&Aa(e.charCodeAt(c));)c++;if(e[c]===";"){let g=e.slice(h,c);g&&(c++,i=String.fromCodePoint(parseInt(g,p)))}}else for(;c<e.length&&o++<10;){let p=e[c];if(c++,p===";"){i=vo.get(r);break}r+=p}return i?{entity:i,newI:c}:{entity:"&",newI:n}}function Aa(e){return e>=u.digit0&&e<=u.digit9}function Pa(e){return e>=u.digit0&&e<=u.digit9||e>=u.lowercaseA&&e<=u.lowercaseF||e>=u.uppercaseA&&e<=u.uppercaseF}function cn(e,n){let r=yt(n),o=new Set;for(let i=0;i<e.tokens.length;i++){let c=e.tokens[i];if(c.type===t.name&&!c.isType&&(c.identifierRole===_.Access||c.identifierRole===_.ObjectShorthand||c.identifierRole===_.ExportAccess)&&!c.shadowsGlobal&&o.add(e.identifierNameForToken(c)),c.type===t.jsxTagStart&&o.add(r.base),c.type===t.jsxTagStart&&i+1<e.tokens.length&&e.tokens[i+1].type===t.jsxTagEnd&&(o.add(r.base),o.add(r.fragmentBase)),c.type===t.jsxName&&c.identifierRole===_.Access){let p=e.identifierNameForToken(c);(!ps(p)||e.tokens[i+1].type===t.dot)&&o.add(e.identifierNameForToken(c))}}return o}var bt=class e{__init(){this.nonTypeIdentifiers=new Set}__init2(){this.importInfoByPath=new Map}__init3(){this.importsToReplace=new Map}__init4(){this.identifierReplacements=new Map}__init5(){this.exportBindingsByLocalName=new Map}constructor(n,r,o,i,c,p,h){this.nameManager=n,this.tokens=r,this.enableLegacyTypeScriptModuleInterop=o,this.options=i,this.isTypeScriptTransformEnabled=c,this.keepUnusedImports=p,this.helperManager=h,e.prototype.__init.call(this),e.prototype.__init2.call(this),e.prototype.__init3.call(this),e.prototype.__init4.call(this),e.prototype.__init5.call(this)}preprocessTokens(){for(let n=0;n<this.tokens.tokens.length;n++)this.tokens.matches1AtIndex(n,t._import)&&!this.tokens.matches3AtIndex(n,t._import,t.name,t.eq)&&this.preprocessImportAtIndex(n),this.tokens.matches1AtIndex(n,t._export)&&!this.tokens.matches2AtIndex(n,t._export,t.eq)&&this.preprocessExportAtIndex(n);this.generateImportReplacements()}pruneTypeOnlyImports(){this.nonTypeIdentifiers=cn(this.tokens,this.options);for(let[n,r]of this.importInfoByPath.entries()){if(r.hasBareImport||r.hasStarExport||r.exportStarNames.length>0||r.namedExports.length>0)continue;[...r.defaultNames,...r.wildcardNames,...r.namedImports.map(({localName:i})=>i)].every(i=>this.shouldAutomaticallyElideImportedName(i))&&this.importsToReplace.set(n,"")}}shouldAutomaticallyElideImportedName(n){return this.isTypeScriptTransformEnabled&&!this.keepUnusedImports&&!this.nonTypeIdentifiers.has(n)}generateImportReplacements(){for(let[n,r]of this.importInfoByPath.entries()){let{defaultNames:o,wildcardNames:i,namedImports:c,namedExports:p,exportStarNames:h,hasStarExport:g}=r;if(o.length===0&&i.length===0&&c.length===0&&p.length===0&&h.length===0&&!g){this.importsToReplace.set(n,`require('${n}');`);continue}let y=this.getFreeIdentifierForPath(n),x;this.enableLegacyTypeScriptModuleInterop?x=y:x=i.length>0?i[0]:this.getFreeIdentifierForPath(n);let v=`var ${y} = require('${n}');`;if(i.length>0)for(let T of i){let E=this.enableLegacyTypeScriptModuleInterop?y:`${this.helperManager.getHelperName("interopRequireWildcard")}(${y})`;v+=` var ${T} = ${E};`}else h.length>0&&x!==y?v+=` var ${x} = ${this.helperManager.getHelperName("interopRequireWildcard")}(${y});`:o.length>0&&x!==y&&(v+=` var ${x} = ${this.helperManager.getHelperName("interopRequireDefault")}(${y});`);for(let{importedName:T,localName:E}of p)v+=` ${this.helperManager.getHelperName("createNamedExportFrom")}(${y}, '${E}', '${T}');`;for(let T of h)v+=` exports.${T} = ${x};`;g&&(v+=` ${this.helperManager.getHelperName("createStarExport")}(${y});`),this.importsToReplace.set(n,v);for(let T of o)this.identifierReplacements.set(T,`${x}.default`);for(let{importedName:T,localName:E}of c)this.identifierReplacements.set(E,`${y}.${T}`)}}getFreeIdentifierForPath(n){let r=n.split("/"),i=r[r.length-1].replace(/\W/g,"");return this.nameManager.claimFreeName(`_${i}`)}preprocessImportAtIndex(n){let r=[],o=[],i=[];if(n++,(this.tokens.matchesContextualAtIndex(n,l._type)||this.tokens.matches1AtIndex(n,t._typeof))&&!this.tokens.matches1AtIndex(n+1,t.comma)&&!this.tokens.matchesContextualAtIndex(n+1,l._from)||this.tokens.matches1AtIndex(n,t.parenL))return;if(this.tokens.matches1AtIndex(n,t.name)&&(r.push(this.tokens.identifierNameAtIndex(n)),n++,this.tokens.matches1AtIndex(n,t.comma)&&n++),this.tokens.matches1AtIndex(n,t.star)&&(n+=2,o.push(this.tokens.identifierNameAtIndex(n)),n++),this.tokens.matches1AtIndex(n,t.braceL)){let h=this.getNamedImports(n+1);n=h.newIndex;for(let g of h.namedImports)g.importedName==="default"?r.push(g.localName):i.push(g)}if(this.tokens.matchesContextualAtIndex(n,l._from)&&n++,!this.tokens.matches1AtIndex(n,t.string))throw new Error("Expected string token at the end of import statement.");let c=this.tokens.stringValueAtIndex(n),p=this.getImportInfo(c);p.defaultNames.push(...r),p.wildcardNames.push(...o),p.namedImports.push(...i),r.length===0&&o.length===0&&i.length===0&&(p.hasBareImport=!0)}preprocessExportAtIndex(n){if(this.tokens.matches2AtIndex(n,t._export,t._var)||this.tokens.matches2AtIndex(n,t._export,t._let)||this.tokens.matches2AtIndex(n,t._export,t._const))this.preprocessVarExportAtIndex(n);else if(this.tokens.matches2AtIndex(n,t._export,t._function)||this.tokens.matches2AtIndex(n,t._export,t._class)){let r=this.tokens.identifierNameAtIndex(n+2);this.addExportBinding(r,r)}else if(this.tokens.matches3AtIndex(n,t._export,t.name,t._function)){let r=this.tokens.identifierNameAtIndex(n+3);this.addExportBinding(r,r)}else this.tokens.matches2AtIndex(n,t._export,t.braceL)?this.preprocessNamedExportAtIndex(n):this.tokens.matches2AtIndex(n,t._export,t.star)&&this.preprocessExportStarAtIndex(n)}preprocessVarExportAtIndex(n){let r=0;for(let o=n+2;;o++)if(this.tokens.matches1AtIndex(o,t.braceL)||this.tokens.matches1AtIndex(o,t.dollarBraceL)||this.tokens.matches1AtIndex(o,t.bracketL))r++;else if(this.tokens.matches1AtIndex(o,t.braceR)||this.tokens.matches1AtIndex(o,t.bracketR))r--;else{if(r===0&&!this.tokens.matches1AtIndex(o,t.name))break;if(this.tokens.matches1AtIndex(1,t.eq)){let i=this.tokens.currentToken().rhsEndIndex;if(i==null)throw new Error("Expected = token with an end index.");o=i-1}else{let i=this.tokens.tokens[o];if(nn(i)){let c=this.tokens.identifierNameAtIndex(o);this.identifierReplacements.set(c,`exports.${c}`)}}}}preprocessNamedExportAtIndex(n){n+=2;let{newIndex:r,namedImports:o}=this.getNamedImports(n);if(n=r,this.tokens.matchesContextualAtIndex(n,l._from))n++;else{for(let{importedName:p,localName:h}of o)this.addExportBinding(p,h);return}if(!this.tokens.matches1AtIndex(n,t.string))throw new Error("Expected string token at the end of import statement.");let i=this.tokens.stringValueAtIndex(n);this.getImportInfo(i).namedExports.push(...o)}preprocessExportStarAtIndex(n){let r=null;if(this.tokens.matches3AtIndex(n,t._export,t.star,t._as)?(n+=3,r=this.tokens.identifierNameAtIndex(n),n+=2):n+=3,!this.tokens.matches1AtIndex(n,t.string))throw new Error("Expected string token at the end of star export statement.");let o=this.tokens.stringValueAtIndex(n),i=this.getImportInfo(o);r!==null?i.exportStarNames.push(r):i.hasStarExport=!0}getNamedImports(n){let r=[];for(;;){if(this.tokens.matches1AtIndex(n,t.braceR)){n++;break}let o=ge(this.tokens,n);if(n=o.endIndex,o.isType||r.push({importedName:o.leftName,localName:o.rightName}),this.tokens.matches2AtIndex(n,t.comma,t.braceR)){n+=2;break}else if(this.tokens.matches1AtIndex(n,t.braceR)){n++;break}else if(this.tokens.matches1AtIndex(n,t.comma))n++;else throw new Error(`Unexpected token: ${JSON.stringify(this.tokens.tokens[n])}`)}return{newIndex:n,namedImports:r}}getImportInfo(n){let r=this.importInfoByPath.get(n);if(r)return r;let o={defaultNames:[],wildcardNames:[],namedImports:[],namedExports:[],hasBareImport:!1,exportStarNames:[],hasStarExport:!1};return this.importInfoByPath.set(n,o),o}addExportBinding(n,r){this.exportBindingsByLocalName.has(n)||this.exportBindingsByLocalName.set(n,[]),this.exportBindingsByLocalName.get(n).push(r)}claimImportCode(n){let r=this.importsToReplace.get(n);return this.importsToReplace.set(n,""),r||""}getIdentifierReplacement(n){return this.identifierReplacements.get(n)||null}resolveExportBinding(n){let r=this.exportBindingsByLocalName.get(n);return!r||r.length===0?null:r.map(o=>`exports.${o}`).join(" = ")}getGlobalNames(){return new Set([...this.identifierReplacements.keys(),...this.exportBindingsByLocalName.keys()])}};var Ca=44,Na=59,Eo="ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/",Co=new Uint8Array(64),Ra=new Uint8Array(128);for(let e=0;e<Eo.length;e++){let n=Eo.charCodeAt(e);Co[e]=n,Ra[n]=e}function _t(e,n){do{let r=n&31;n>>>=5,n>0&&(r|=32),e.write(Co[r])}while(n>0)}function wt(e){return e<0?-e<<1|1:e<<1}var Ao=1024*16,Po=typeof TextDecoder<"u"?new TextDecoder:typeof Buffer<"u"?{decode(e){return Buffer.from(e.buffer,e.byteOffset,e.byteLength).toString()}}:{decode(e){let n="";for(let r=0;r<e.length;r++)n+=String.fromCharCode(e[r]);return n}},La=class{constructor(){this.pos=0,this.out="",this.buffer=new Uint8Array(Ao)}write(e){let{buffer:n}=this;n[this.pos++]=e,this.pos===Ao&&(this.out+=Po.decode(n),this.pos=0)}flush(){let{buffer:e,out:n,pos:r}=this;return r>0?n+Po.decode(e.subarray(0,r)):n}};function fs(e){let n=new La,r=0,o=0,i=0,c=0;for(let p=0;p<e.length;p++){let h=e[p];if(p>0&&n.write(Na),h.length===0)continue;let g=0;for(let y=0;y<h.length;y++){let x=h[y];y>0&&n.write(Ca),_t(n,wt(x[0]-g)),g=x[0],x.length!==1&&(_t(n,wt(x[1]-r)),_t(n,wt(x[2]-o)),_t(n,wt(x[3]-i)),r=x[1],o=x[2],i=x[3],x.length!==4&&(_t(n,wt(x[4]-c)),c=x[4]))}}return n.flush()}var hs=class{constructor(){this._indexes={__proto__:null},this.array=[]}};function Da(e,n){return e._indexes[n]}function No(e,n){let r=Da(e,n);if(r!==void 0)return r;let{array:o,_indexes:i}=e,c=o.push(n);return i[n]=c-1}var Oa=0,Ma=1,ja=2,Fa=3,Ba=4,Lo=-1,Do=class{constructor({file:e,sourceRoot:n}={}){this._names=new hs,this._sources=new hs,this._sourcesContent=[],this._mappings=[],this.file=e,this.sourceRoot=n,this._ignoreList=new hs}};var ln=(e,n,r,o,i,c,p,h)=>$a(!0,e,n,r,o,i,c,p,h);function qa(e){let{_mappings:n,_sources:r,_sourcesContent:o,_names:i,_ignoreList:c}=e;return Va(n),{version:3,file:e.file||void 0,names:i.array,sourceRoot:e.sourceRoot||void 0,sources:r.array,sourcesContent:o,mappings:n,ignoreList:c.array}}function Oo(e){let n=qa(e);return Object.assign({},n,{mappings:fs(n.mappings)})}function $a(e,n,r,o,i,c,p,h,g){let{_mappings:y,_sources:x,_sourcesContent:v,_names:T}=n,E=Ua(y,r),O=Ha(E,o);if(!i)return e&&Wa(E,O)?void 0:Ro(E,O,[o]);let q=No(x,i),G=h?No(T,h):Lo;if(q===v.length&&(v[q]=g??null),!(e&&za(E,O,q,c,p,G)))return Ro(E,O,h?[o,q,c,p,G]:[o,q,c,p])}function Ua(e,n){for(let r=e.length;r<=n;r++)e[r]=[];return e[n]}function Ha(e,n){let r=e.length;for(let o=r-1;o>=0;r=o--){let i=e[o];if(n>=i[Oa])break}return r}function Ro(e,n,r){for(let o=e.length;o>n;o--)e[o]=e[o-1];e[n]=r}function Va(e){let{length:n}=e,r=n;for(let o=r-1;o>=0&&!(e[o].length>0);r=o,o--);r<n&&(e.length=r)}function Wa(e,n){return n===0?!0:e[n-1].length===1}function za(e,n,r,o,i,c){if(n===0)return!1;let p=e[n-1];return p.length===1?!1:r===p[Ma]&&o===p[ja]&&i===p[Fa]&&c===(p.length===5?p[Ba]:Lo)}function ds({code:e,mappings:n},r,o,i,c){let p=Xa(i,c),h=new Do({file:o.compiledFilename}),g=0,y=n[0];for(;y===void 0&&g<n.length-1;)g++,y=n[g];let x=0,v=0;y!==v&&ln(h,x,0,r,x,0);for(let q=0;q<e.length;q++){if(q===y){let G=y-v,N=p[g];for(ln(h,x,G,r,x,N);(y===q||y===void 0)&&g<n.length-1;)g++,y=n[g]}e.charCodeAt(q)===u.lineFeed&&(x++,v=q+1,y!==v&&ln(h,x,0,r,x,0))}let{sourceRoot:T,sourcesContent:E,...O}=Oo(h);return O}function Xa(e,n){let r=new Array(n.length),o=0,i=n[o].start,c=0;for(let p=0;p<e.length;p++)p===i&&(r[o]=i-c,o++,i=n[o].start),e.charCodeAt(p)===u.lineFeed&&(c=p+1);return r}var Ja={require:`
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
  `},un=class e{__init(){this.helperNames={}}__init2(){this.createRequireName=null}constructor(n){this.nameManager=n,e.prototype.__init.call(this),e.prototype.__init2.call(this)}getHelperName(n){let r=this.helperNames[n];return r||(r=this.nameManager.claimFreeName(`_${n}`),this.helperNames[n]=r,r)}emitHelpers(){let n="";this.helperNames.optionalChainDelete&&this.getHelperName("optionalChain"),this.helperNames.asyncOptionalChainDelete&&this.getHelperName("asyncOptionalChain");for(let[r,o]of Object.entries(Ja)){let i=this.helperNames[r],c=o;r==="optionalChainDelete"?c=c.replace("OPTIONAL_CHAIN_NAME",this.helperNames.optionalChain):r==="asyncOptionalChainDelete"?c=c.replace("ASYNC_OPTIONAL_CHAIN_NAME",this.helperNames.asyncOptionalChain):r==="require"&&(this.createRequireName===null&&(this.createRequireName=this.nameManager.claimFreeName("_createRequire")),c=c.replace(/CREATE_REQUIRE_NAME/g,this.createRequireName)),i&&(n+=" ",n+=c.replace(r,i).replace(/\s+/g," ").trim())}return n}};function pn(e,n,r){Ga(e,r)&&Ka(e,n,r)}function Ga(e,n){for(let r of e.tokens)if(r.type===t.name&&!r.isType&&go(r)&&n.has(e.identifierNameForToken(r)))return!0;return!1}function Ka(e,n,r){let o=[],i=n.length-1;for(let c=e.tokens.length-1;;c--){for(;o.length>0&&o[o.length-1].startTokenIndex===c+1;)o.pop();for(;i>=0&&n[i].endTokenIndex===c+1;)o.push(n[i]),i--;if(c<0)break;let p=e.tokens[c],h=e.identifierNameForToken(p);if(o.length>1&&!p.isType&&p.type===t.name&&r.has(h)){if(ko(p))Mo(o[o.length-1],e,h);else if(yo(p)){let g=o.length-1;for(;g>0&&!o[g].isFunctionScope;)g--;if(g<0)throw new Error("Did not find parent function scope.");Mo(o[g],e,h)}}}if(o.length>0)throw new Error("Expected empty scope stack after processing file.")}function Mo(e,n,r){for(let o=e.startTokenIndex;o<e.endTokenIndex;o++){let i=n.tokens[o];(i.type===t.name||i.type===t.jsxName)&&n.identifierNameForToken(i)===r&&(i.shadowsGlobal=!0)}}function ms(e,n){let r=[];for(let o of n)o.type===t.name&&r.push(e.slice(o.start,o.end));return r}var vt=class e{__init(){this.usedNames=new Set}constructor(n,r){e.prototype.__init.call(this),this.usedNames=new Set(ms(n,r))}claimFreeName(n){let r=this.findFreeName(n);return this.usedNames.add(r),r}findFreeName(n){if(!this.usedNames.has(n))return n;let r=2;for(;this.usedNames.has(n+String(r));)r++;return n+String(r)}};var ei=Jr(Ts());var j=Jr(Ts()),yc=j.union(j.lit("jsx"),j.lit("typescript"),j.lit("flow"),j.lit("imports"),j.lit("react-hot-loader"),j.lit("jest")),xc=j.iface([],{compiledFilename:"string"}),bc=j.iface([],{transforms:j.array("Transform"),disableESTransforms:j.opt("boolean"),jsxRuntime:j.opt(j.union(j.lit("classic"),j.lit("automatic"),j.lit("preserve"))),production:j.opt("boolean"),jsxImportSource:j.opt("string"),jsxPragma:j.opt("string"),jsxFragmentPragma:j.opt("string"),keepUnusedImports:j.opt("boolean"),preserveDynamicImport:j.opt("boolean"),injectCreateRequireForImportRequire:j.opt("boolean"),enableLegacyTypeScriptModuleInterop:j.opt("boolean"),enableLegacyBabel5ModuleInterop:j.opt("boolean"),sourceMapOptions:j.opt("SourceMapOptions"),filePath:j.opt("string")}),_c={Transform:yc,SourceMapOptions:xc,Options:bc},Zo=_c;var{Options:wc}=(0,ei.createCheckers)(Zo);function ti(e){wc.strictCheck(e)}function Is(){m(),z(!1)}function Ss(e){m(),St(e)}function he(e){I(),dn(e)}function it(){I(),s.tokens[s.tokens.length-1].identifierRole=_.ImportDeclaration}function dn(e){let n;s.scopeDepth===0?n=_.TopLevelDeclaration:e?n=_.BlockScopedDeclaration:n=_.FunctionScopedDeclaration,s.tokens[s.tokens.length-1].identifierRole=n}function St(e){switch(s.type){case t._this:{let n=C(0);m(),P(n);return}case t._yield:case t.name:{s.type=t.name,he(e);return}case t.bracketL:{m(),Et(t.bracketR,e,!0);return}case t.braceL:Pt(!0,e);return;default:S()}}function Et(e,n,r=!1,o=!1,i=0){let c=!0,p=!1,h=s.tokens.length;for(;!f(e)&&!s.error;)if(c?c=!1:(d(t.comma),s.tokens[s.tokens.length-1].contextId=i,!p&&s.tokens[h].isType&&(s.tokens[s.tokens.length-1].isType=!0,p=!0)),!(r&&a(t.comma))){if(f(e))break;if(a(t.ellipsis)){Ss(n),ni(),f(t.comma),d(e);break}else vc(o,n)}}function vc(e,n){e&&At([l._public,l._protected,l._private,l._readonly,l._override]),It(n),ni(),It(n,!0)}function ni(){L?si():R&&ri()}function It(e,n=!1){if(n||St(e),!f(t.eq))return;let r=s.tokens.length-1;z(),s.tokens[r].rhsEndIndex=s.tokens.length}function As(){return a(t.name)}function Tc(){return a(t.name)||!!(s.type&t.IS_KEYWORD)||a(t.string)||a(t.num)||a(t.bigint)||a(t.decimal)}function ui(){let e=s.snapshot();return m(),(a(t.bracketL)||a(t.braceL)||a(t.star)||a(t.ellipsis)||a(t.hash)||Tc())&&!K()?!0:(s.restoreFromSnapshot(e),!1)}function At(e){for(;pi(e)!==null;);}function pi(e){if(!a(t.name))return null;let n=s.contextualKeyword;if(e.indexOf(n)!==-1&&ui()){switch(n){case l._readonly:s.tokens[s.tokens.length-1].type=t._readonly;break;case l._abstract:s.tokens[s.tokens.length-1].type=t._abstract;break;case l._static:s.tokens[s.tokens.length-1].type=t._static;break;case l._public:s.tokens[s.tokens.length-1].type=t._public;break;case l._private:s.tokens[s.tokens.length-1].type=t._private;break;case l._protected:s.tokens[s.tokens.length-1].type=t._protected;break;case l._override:s.tokens[s.tokens.length-1].type=t._override;break;case l._declare:s.tokens[s.tokens.length-1].type=t._declare;break;default:break}return n}return null}function Nt(){for(I();f(t.dot);)I()}function Ic(){Nt(),!K()&&a(t.lessThan)&&lt()}function Sc(){m(),ct()}function Ec(){m()}function Ac(){d(t._typeof),a(t._import)?fi():Nt(),!K()&&a(t.lessThan)&&lt()}function fi(){d(t._import),d(t.parenL),d(t.string),d(t.parenR),f(t.dot)&&Nt(),a(t.lessThan)&&lt()}function Pc(){f(t._const);let e=f(t._in),n=W(l._out);f(t._const),(e||n)&&!a(t.name)?s.tokens[s.tokens.length-1].type=t.name:I(),f(t._extends)&&V(),f(t.eq)&&V()}function Ce(){a(t.lessThan)&&gn()}function gn(){let e=C(0);for(a(t.lessThan)||a(t.typeParameterStart)?m():S();!f(t.greaterThan)&&!s.error;)Pc(),f(t.comma);P(e)}function Ns(e){let n=e===t.arrow;Ce(),d(t.parenL),s.scopeDepth++,Cc(!1),s.scopeDepth--,(n||a(e))&&Ct(e)}function Cc(e){Et(t.parenR,e)}function mn(){f(t.comma)||F()}function oi(){Ns(t.colon),mn()}function Nc(){let e=s.snapshot();m();let n=f(t.name)&&a(t.colon);return s.restoreFromSnapshot(e),n}function hi(){if(!(a(t.bracketL)&&Nc()))return!1;let e=C(0);return d(t.bracketL),I(),ct(),d(t.bracketR),Le(),mn(),P(e),!0}function ii(e){f(t.question),!e&&(a(t.parenL)||a(t.lessThan))?(Ns(t.colon),mn()):(Le(),mn())}function Rc(){if(a(t.parenL)||a(t.lessThan)){oi();return}if(a(t._new)){m(),a(t.parenL)||a(t.lessThan)?oi():ii(!1);return}let e=!!pi([l._readonly]);hi()||((b(l._get)||b(l._set))&&ui(),De(-1),ii(e))}function Lc(){di()}function di(){for(d(t.braceL);!f(t.braceR)&&!s.error;)Rc()}function Dc(){let e=s.snapshot(),n=Oc();return s.restoreFromSnapshot(e),n}function Oc(){return m(),f(t.plus)||f(t.minus)?b(l._readonly):(b(l._readonly)&&m(),!a(t.bracketL)||(m(),!As())?!1:(m(),a(t._in)))}function Mc(){I(),d(t._in),V()}function jc(){d(t.braceL),a(t.plus)||a(t.minus)?(m(),U(l._readonly)):W(l._readonly),d(t.bracketL),Mc(),W(l._as)&&V(),d(t.bracketR),a(t.plus)||a(t.minus)?(m(),d(t.question)):f(t.question),Yc(),F(),d(t.braceR)}function Fc(){for(d(t.bracketL);!f(t.bracketR)&&!s.error;)Bc(),f(t.comma)}function Bc(){f(t.ellipsis)?V():(V(),f(t.question)),f(t.colon)&&V()}function qc(){d(t.parenL),V(),d(t.parenR)}function $c(){for(me(),me();!a(t.backQuote)&&!s.error;)d(t.dollarBraceL),V(),me(),me();m()}var Ae;(function(e){e[e.TSFunctionType=0]="TSFunctionType";let r=1;e[e.TSConstructorType=r]="TSConstructorType";let o=r+1;e[e.TSAbstractConstructorType=o]="TSAbstractConstructorType"})(Ae||(Ae={}));function Es(e){e===Ae.TSAbstractConstructorType&&U(l._abstract),(e===Ae.TSConstructorType||e===Ae.TSAbstractConstructorType)&&d(t._new);let n=s.inDisallowConditionalTypesContext;s.inDisallowConditionalTypesContext=!1,Ns(t.arrow),s.inDisallowConditionalTypesContext=n}function Uc(){switch(s.type){case t.name:Ic();return;case t._void:case t._null:m();return;case t.string:case t.num:case t.bigint:case t.decimal:case t._true:case t._false:Pe();return;case t.minus:m(),Pe();return;case t._this:{Ec(),b(l._is)&&!K()&&Sc();return}case t._typeof:Ac();return;case t._import:fi();return;case t.braceL:Dc()?jc():Lc();return;case t.bracketL:Fc();return;case t.parenL:qc();return;case t.backQuote:$c();return;default:if(s.type&t.IS_KEYWORD){m(),s.tokens[s.tokens.length-1].type=t.name;return}break}S()}function Hc(){for(Uc();!K()&&f(t.bracketL);)f(t.bracketR)||(V(),d(t.bracketR))}function Vc(){if(U(l._infer),I(),a(t._extends)){let e=s.snapshot();d(t._extends);let n=s.inDisallowConditionalTypesContext;s.inDisallowConditionalTypesContext=!0,V(),s.inDisallowConditionalTypesContext=n,(s.error||!s.inDisallowConditionalTypesContext&&a(t.question))&&s.restoreFromSnapshot(e)}}function Ps(){if(b(l._keyof)||b(l._unique)||b(l._readonly))m(),Ps();else if(b(l._infer))Vc();else{let e=s.inDisallowConditionalTypesContext;s.inDisallowConditionalTypesContext=!1,Hc(),s.inDisallowConditionalTypesContext=e}}function ai(){if(f(t.bitwiseAND),Ps(),a(t.bitwiseAND))for(;f(t.bitwiseAND);)Ps()}function Wc(){if(f(t.bitwiseOR),ai(),a(t.bitwiseOR))for(;f(t.bitwiseOR);)ai()}function zc(){return a(t.lessThan)?!0:a(t.parenL)&&Jc()}function Xc(){if(a(t.name)||a(t._this))return m(),!0;if(a(t.braceL)||a(t.bracketL)){let e=1;for(m();e>0&&!s.error;)a(t.braceL)||a(t.bracketL)?e++:(a(t.braceR)||a(t.bracketR))&&e--,m();return!0}return!1}function Jc(){let e=s.snapshot(),n=Gc();return s.restoreFromSnapshot(e),n}function Gc(){return m(),!!(a(t.parenR)||a(t.ellipsis)||Xc()&&(a(t.colon)||a(t.comma)||a(t.question)||a(t.eq)||a(t.parenR)&&(m(),a(t.arrow))))}function Ct(e){let n=C(0);d(e),Qc()||V(),P(n)}function Kc(){a(t.colon)&&Ct(t.colon)}function Le(){a(t.colon)&&ct()}function Yc(){f(t.colon)&&V()}function Qc(){let e=s.snapshot();return b(l._asserts)?(m(),W(l._is)?(V(),!0):As()||a(t._this)?(m(),W(l._is)&&V(),!0):(s.restoreFromSnapshot(e),!1)):As()||a(t._this)?(m(),b(l._is)&&!K()?(m(),V(),!0):(s.restoreFromSnapshot(e),!1)):!1}function ct(){let e=C(0);d(t.colon),V(),P(e)}function V(){if(ci(),s.inDisallowConditionalTypesContext||K()||!f(t._extends))return;let e=s.inDisallowConditionalTypesContext;s.inDisallowConditionalTypesContext=!0,ci(),s.inDisallowConditionalTypesContext=e,d(t.question),V(),d(t.colon),V()}function Zc(){return b(l._abstract)&&B()===t._new}function ci(){if(zc()){Es(Ae.TSFunctionType);return}if(a(t._new)){Es(Ae.TSConstructorType);return}else if(Zc()){Es(Ae.TSAbstractConstructorType);return}Wc()}function mi(){let e=C(1);V(),d(t.greaterThan),P(e),ut()}function gi(){if(f(t.jsxTagStart)){s.tokens[s.tokens.length-1].type=t.typeParameterStart;let e=C(1);for(;!a(t.greaterThan)&&!s.error;)V(),f(t.comma);ie(),P(e)}}function ki(){for(;!a(t.braceL)&&!s.error;)el(),f(t.comma)}function el(){Nt(),a(t.lessThan)&&lt()}function tl(){he(!1),Ce(),f(t._extends)&&ki(),di()}function nl(){he(!1),Ce(),d(t.eq),V(),F()}function rl(){if(a(t.string)?Pe():I(),f(t.eq)){let e=s.tokens.length-1;z(),s.tokens[e].rhsEndIndex=s.tokens.length}}function Rs(){for(he(!1),d(t.braceL);!f(t.braceR)&&!s.error;)rl(),f(t.comma)}function Ls(){d(t.braceL),pt(t.braceR)}function Cs(){he(!1),f(t.dot)?Cs():Ls()}function yi(){b(l._global)?I():a(t.string)?pe():S(),a(t.braceL)?Ls():F()}function kn(){it(),d(t.eq),ol(),F()}function sl(){return b(l._require)&&B()===t.parenL}function ol(){sl()?il():Nt()}function il(){U(l._require),d(t.parenL),a(t.string)||S(),Pe(),d(t.parenR)}function al(){if(le())return!1;switch(s.type){case t._function:{let e=C(1);m();let n=s.start;return _e(n,!0),P(e),!0}case t._class:{let e=C(1);return ve(!0,!1),P(e),!0}case t._const:if(a(t._const)&&st(l._enum)){let e=C(1);return d(t._const),U(l._enum),s.tokens[s.tokens.length-1].type=t._enum,Rs(),P(e),!0}case t._var:case t._let:{let e=C(1);return Lt(s.type!==t._var),P(e),!0}case t.name:{let e=C(1),n=s.contextualKeyword,r=!1;return n===l._global?(yi(),r=!0):r=yn(n,!0),P(e),r}default:return!1}}function li(){return yn(s.contextualKeyword,!0)}function cl(e){switch(e){case l._declare:{let n=s.tokens.length-1;if(al())return s.tokens[n].type=t._declare,!0;break}case l._global:if(a(t.braceL))return Ls(),!0;break;default:return yn(e,!1)}return!1}function yn(e,n){switch(e){case l._abstract:if(at(n)&&a(t._class))return s.tokens[s.tokens.length-1].type=t._abstract,ve(!0,!1),!0;break;case l._enum:if(at(n)&&a(t.name))return s.tokens[s.tokens.length-1].type=t._enum,Rs(),!0;break;case l._interface:if(at(n)&&a(t.name)){let r=C(n?2:1);return tl(),P(r),!0}break;case l._module:if(at(n)){if(a(t.string)){let r=C(n?2:1);return yi(),P(r),!0}else if(a(t.name)){let r=C(n?2:1);return Cs(),P(r),!0}}break;case l._namespace:if(at(n)&&a(t.name)){let r=C(n?2:1);return Cs(),P(r),!0}break;case l._type:if(at(n)&&a(t.name)){let r=C(n?2:1);return nl(),P(r),!0}break;default:break}return!1}function at(e){return e?(m(),!0):!le()}function ll(){let e=s.snapshot();return gn(),we(),Kc(),d(t.arrow),s.error?(s.restoreFromSnapshot(e),!1):(Oe(!0),!0)}function Ds(){s.type===t.bitShiftL&&(s.pos-=1,A(t.lessThan)),lt()}function lt(){let e=C(0);for(d(t.lessThan);!a(t.greaterThan)&&!s.error;)V(),f(t.comma);e?(d(t.greaterThan),P(e)):(P(e),on(),d(t.greaterThan),s.tokens[s.tokens.length-1].isType=!0)}function Os(){if(a(t.name))switch(s.contextualKeyword){case l._abstract:case l._declare:case l._enum:case l._interface:case l._module:case l._namespace:case l._type:return!0;default:break}return!1}function xi(e,n){if(a(t.colon)&&Ct(t.colon),!a(t.braceL)&&le()){let r=s.tokens.length-1;for(;r>=0&&(s.tokens[r].start>=e||s.tokens[r].type===t._default||s.tokens[r].type===t._export);)s.tokens[r].isType=!0,r--;return}Oe(!1,n)}function bi(e,n,r){if(!K()&&f(t.bang)){s.tokens[s.tokens.length-1].type=t.nonNullAssertion;return}if(a(t.lessThan)||a(t.bitShiftL)){let o=s.snapshot();if(!n&&Ms()&&ll())return;if(Ds(),!n&&f(t.parenL)?(s.tokens[s.tokens.length-1].subscriptStartIndex=e,de()):a(t.backQuote)?xn():(s.type===t.greaterThan||s.type!==t.parenL&&s.type&t.IS_EXPRESSION_START&&!K())&&S(),s.error)s.restoreFromSnapshot(o);else return}else!n&&a(t.questionDot)&&B()===t.lessThan&&(m(),s.tokens[e].isOptionalChainStart=!0,s.tokens[s.tokens.length-1].subscriptStartIndex=e,lt(),d(t.parenL),de());Rt(e,n,r)}function _i(){if(f(t._import))return b(l._type)&&B()!==t.eq&&U(l._type),kn(),!0;if(f(t.eq))return X(),F(),!0;if(W(l._as))return U(l._namespace),I(),F(),!0;if(b(l._type)){let e=B();(e===t.braceL||e===t.star)&&m()}return!1}function wi(){if(I(),a(t.comma)||a(t.braceR)){s.tokens[s.tokens.length-1].identifierRole=_.ImportDeclaration;return}if(I(),a(t.comma)||a(t.braceR)){s.tokens[s.tokens.length-1].identifierRole=_.ImportDeclaration,s.tokens[s.tokens.length-2].isType=!0,s.tokens[s.tokens.length-1].isType=!0;return}if(I(),a(t.comma)||a(t.braceR)){s.tokens[s.tokens.length-3].identifierRole=_.ImportAccess,s.tokens[s.tokens.length-1].identifierRole=_.ImportDeclaration;return}I(),s.tokens[s.tokens.length-3].identifierRole=_.ImportAccess,s.tokens[s.tokens.length-1].identifierRole=_.ImportDeclaration,s.tokens[s.tokens.length-4].isType=!0,s.tokens[s.tokens.length-3].isType=!0,s.tokens[s.tokens.length-2].isType=!0,s.tokens[s.tokens.length-1].isType=!0}function vi(){if(I(),a(t.comma)||a(t.braceR)){s.tokens[s.tokens.length-1].identifierRole=_.ExportAccess;return}if(I(),a(t.comma)||a(t.braceR)){s.tokens[s.tokens.length-1].identifierRole=_.ExportAccess,s.tokens[s.tokens.length-2].isType=!0,s.tokens[s.tokens.length-1].isType=!0;return}if(I(),a(t.comma)||a(t.braceR)){s.tokens[s.tokens.length-3].identifierRole=_.ExportAccess;return}I(),s.tokens[s.tokens.length-3].identifierRole=_.ExportAccess,s.tokens[s.tokens.length-4].isType=!0,s.tokens[s.tokens.length-3].isType=!0,s.tokens[s.tokens.length-2].isType=!0,s.tokens[s.tokens.length-1].isType=!0}function Ti(){if(b(l._abstract)&&B()===t._class)return s.type=t._abstract,m(),ve(!0,!0),!0;if(b(l._interface)){let e=C(2);return yn(l._interface,!0),P(e),!0}return!1}function Ii(){if(s.type===t._const){let e=xe();if(e.type===t.name&&e.contextualKeyword===l._enum)return d(t._const),U(l._enum),s.tokens[s.tokens.length-1].type=t._enum,Rs(),!0}return!1}function Si(e){let n=s.tokens.length;At([l._abstract,l._readonly,l._declare,l._static,l._override]);let r=s.tokens.length;if(hi()){let i=e?n-1:n;for(let c=i;c<r;c++)s.tokens[c].isType=!0;return!0}return!1}function Ei(e){cl(e)||F()}function Ai(){let e=W(l._declare);e&&(s.tokens[s.tokens.length-1].type=t._declare);let n=!1;if(a(t.name))if(e){let r=C(2);n=li(),P(r)}else n=li();if(!n)if(e){let r=C(2);ee(!0),P(r)}else ee(!0)}function Pi(e){if(e&&(a(t.lessThan)||a(t.bitShiftL))&&Ds(),W(l._implements)){s.tokens[s.tokens.length-1].type=t._implements;let n=C(1);ki(),P(n)}}function Ci(){Ce()}function Ni(){Ce()}function Ri(){let e=C(0);K()||f(t.bang),Le(),P(e)}function Li(){a(t.colon)&&ct()}function Di(e,n){return rt?ul(e,n):pl(e,n)}function ul(e,n){if(!a(t.lessThan))return ue(e,n);let r=s.snapshot(),o=ue(e,n);if(s.error)s.restoreFromSnapshot(r);else return o;return s.type=t.typeParameterStart,gn(),o=ue(e,n),o||S(),o}function pl(e,n){if(!a(t.lessThan))return ue(e,n);let r=s.snapshot();gn();let o=ue(e,n);if(o||S(),s.error)s.restoreFromSnapshot(r);else return o;return ue(e,n)}function Oi(){if(a(t.colon)){let e=s.snapshot();Ct(t.colon),Y()&&S(),a(t.arrow)||S(),s.error&&s.restoreFromSnapshot(e)}return f(t.arrow)}function ri(){let e=C(0);f(t.question),Le(),P(e)}function Mi(){(a(t.lessThan)||a(t.bitShiftL))&&Ds(),js()}function fl(){let e=!1,n=!1;for(;;){if(s.pos>=k.length){S("Unterminated JSX contents");return}let r=k.charCodeAt(s.pos);if(r===u.lessThan||r===u.leftCurlyBrace){if(s.pos===s.start){if(r===u.lessThan){s.pos++,A(t.jsxTagStart);return}us(r);return}e&&!n?A(t.jsxEmptyText):A(t.jsxText);return}r===u.lineFeed?e=!0:r!==u.space&&r!==u.carriageReturn&&r!==u.tab&&(n=!0),s.pos++}}function hl(e){for(s.pos++;;){if(s.pos>=k.length){S("Unterminated string constant");return}if(k.charCodeAt(s.pos)===e){s.pos++;break}s.pos++}A(t.string)}function dl(){let e;do{if(s.pos>k.length){S("Unexpectedly reached the end of input.");return}e=k.charCodeAt(++s.pos)}while(Z[e]||e===u.dash);A(t.jsxName)}function Fs(){ie()}function ji(e){if(Fs(),!f(t.colon)){s.tokens[s.tokens.length-1].identifierRole=e;return}Fs()}function Fi(){let e=s.tokens.length;ji(_.Access);let n=!1;for(;a(t.dot);)n=!0,ie(),Fs();if(!n){let r=s.tokens[e],o=k.charCodeAt(r.start);o>=u.lowercaseA&&o<=u.lowercaseZ&&(r.identifierRole=null)}}function ml(){switch(s.type){case t.braceL:m(),X(),ie();return;case t.jsxTagStart:Bs(),ie();return;case t.string:ie();return;default:S("JSX value should be either an expression or a quoted JSX text")}}function gl(){d(t.ellipsis),X()}function kl(e){if(a(t.jsxTagEnd))return!1;Fi(),R&&gi();let n=!1;for(;!a(t.slash)&&!a(t.jsxTagEnd)&&!s.error;){if(f(t.braceL)){n=!0,d(t.ellipsis),z(),ie();continue}n&&s.end-s.start===3&&k.charCodeAt(s.start)===u.lowercaseK&&k.charCodeAt(s.start+1)===u.lowercaseE&&k.charCodeAt(s.start+2)===u.lowercaseY&&(s.tokens[e].jsxRole=se.KeyAfterPropSpread),ji(_.ObjectKey),a(t.eq)&&(ie(),ml())}let r=a(t.slash);return r&&ie(),r}function yl(){a(t.jsxTagEnd)||Fi()}function Bi(){let e=s.tokens.length-1;s.tokens[e].jsxRole=se.NoChildren;let n=0;if(!kl(e))for(ft();;)switch(s.type){case t.jsxTagStart:if(ie(),a(t.slash)){ie(),yl(),s.tokens[e].jsxRole!==se.KeyAfterPropSpread&&(n===1?s.tokens[e].jsxRole=se.OneChild:n>1&&(s.tokens[e].jsxRole=se.StaticChildren));return}n++,Bi(),ft();break;case t.jsxText:n++,ft();break;case t.jsxEmptyText:ft();break;case t.braceL:m(),a(t.ellipsis)?(gl(),ft(),n+=2):(a(t.braceR)||(n++,X()),ft());break;default:S();return}}function Bs(){ie(),Bi()}function ie(){s.tokens.push(new Re),ls(),s.start=s.pos;let e=k.charCodeAt(s.pos);if(be[e])dl();else if(e===u.quotationMark||e===u.apostrophe)hl(e);else switch(++s.pos,e){case u.greaterThan:A(t.jsxTagEnd);break;case u.lessThan:A(t.jsxTagStart);break;case u.slash:A(t.slash);break;case u.equalsTo:A(t.eq);break;case u.leftCurlyBrace:A(t.braceL);break;case u.dot:A(t.dot);break;case u.colon:A(t.colon);break;default:S()}}function ft(){s.tokens.push(new Re),s.start=s.pos,fl()}function qi(e){if(a(t.question)){let n=B();if(n===t.colon||n===t.comma||n===t.parenR)return}qs(e)}function $i(){sn(t.question),a(t.colon)&&(R?ct():L&&Te())}var $s=class{constructor(n){this.stop=n}};function X(e=!1){if(z(e),a(t.comma))for(;f(t.comma);)z(e)}function z(e=!1,n=!1){return R?Di(e,n):L?Ki(e,n):ue(e,n)}function ue(e,n){if(a(t._yield))return Dl(),!1;(a(t.parenL)||a(t.name)||a(t._yield))&&(s.potentialArrowAt=s.start);let r=xl(e);return n&&zs(),s.type&t.IS_ASSIGN?(m(),z(e),!1):r}function xl(e){return _l(e)?!0:(bl(e),!1)}function bl(e){R||L?qi(e):qs(e)}function qs(e){f(t.question)&&(z(),d(t.colon),z(e))}function _l(e){let n=s.tokens.length;return ut()?!0:(bn(n,-1,e),!1)}function bn(e,n,r){if(R&&(t._in&t.PRECEDENCE_MASK)>n&&!K()&&(W(l._as)||W(l._satisfies))){let i=C(1);V(),P(i),on(),bn(e,n,r);return}let o=s.type&t.PRECEDENCE_MASK;if(o>0&&(!r||!a(t._in))&&o>n){let i=s.type;m(),i===t.nullishCoalescing&&(s.tokens[s.tokens.length-1].nullishStartIndex=e);let c=s.tokens.length;ut(),bn(c,i&t.IS_RIGHT_ASSOCIATIVE?o-1:o,r),i===t.nullishCoalescing&&(s.tokens[e].numNullishCoalesceStarts++,s.tokens[s.tokens.length-1].numNullishCoalesceEnds++),bn(e,n,r)}}function ut(){if(R&&!rt&&f(t.lessThan))return mi(),!1;if(b(l._module)&&is()===u.leftCurlyBrace&&!tn())return Ol(),!1;if(s.type&t.IS_PREFIX)return m(),ut(),!1;if(Us())return!0;for(;s.type&t.IS_POSTFIX&&!Y();)s.type===t.preIncDec&&(s.type=t.postIncDec),m();return!1}function Us(){let e=s.tokens.length;return pe()?!0:(Hs(e),s.tokens.length>e&&s.tokens[e].isOptionalChainStart&&(s.tokens[s.tokens.length-1].isOptionalChainEnd=!0),!1)}function Hs(e,n=!1){L?Qi(e,n):Vs(e,n)}function Vs(e,n=!1){let r=new $s(!1);do wl(e,n,r);while(!r.stop&&!s.error)}function wl(e,n,r){R?bi(e,n,r):L?zi(e,n,r):Rt(e,n,r)}function Rt(e,n,r){if(!n&&f(t.doubleColon))Ws(),r.stop=!0,Hs(e,n);else if(a(t.questionDot)){if(s.tokens[e].isOptionalChainStart=!0,n&&B()===t.parenL){r.stop=!0;return}m(),s.tokens[s.tokens.length-1].subscriptStartIndex=e,f(t.bracketL)?(X(),d(t.bracketR)):f(t.parenL)?de():_n()}else if(f(t.dot))s.tokens[s.tokens.length-1].subscriptStartIndex=e,_n();else if(f(t.bracketL))s.tokens[s.tokens.length-1].subscriptStartIndex=e,X(),d(t.bracketR);else if(!n&&a(t.parenL))if(Ms()){let o=s.snapshot(),i=s.tokens.length;m(),s.tokens[s.tokens.length-1].subscriptStartIndex=e;let c=Ne();s.tokens[s.tokens.length-1].contextId=c,de(),s.tokens[s.tokens.length-1].contextId=c,vl()&&(s.restoreFromSnapshot(o),r.stop=!0,s.scopeDepth++,we(),Tl(i))}else{m(),s.tokens[s.tokens.length-1].subscriptStartIndex=e;let o=Ne();s.tokens[s.tokens.length-1].contextId=o,de(),s.tokens[s.tokens.length-1].contextId=o}else a(t.backQuote)?xn():r.stop=!0}function Ms(){return s.tokens[s.tokens.length-1].contextualKeyword===l._async&&!Y()}function de(){let e=!0;for(;!f(t.parenR)&&!s.error;){if(e)e=!1;else if(d(t.comma),f(t.parenR))break;Vi(!1)}}function vl(){return a(t.colon)||a(t.arrow)}function Tl(e){R?Li():L&&Gi(),d(t.arrow),ht(e)}function Ws(){let e=s.tokens.length;pe(),Hs(e,!0)}function pe(){if(f(t.modulo))return I(),!1;if(a(t.jsxText)||a(t.jsxEmptyText))return Pe(),!1;if(a(t.lessThan)&&rt)return s.type=t.jsxTagStart,Bs(),m(),!1;let e=s.potentialArrowAt===s.start;switch(s.type){case t.slash:case t.assign:bo();case t._super:case t._this:case t.regexp:case t.num:case t.bigint:case t.decimal:case t.string:case t._null:case t._true:case t._false:return m(),!1;case t._import:return m(),a(t.dot)&&(s.tokens[s.tokens.length-1].type=t.name,m(),I()),!1;case t.name:{let n=s.tokens.length,r=s.start,o=s.contextualKeyword;return I(),o===l._await?(Ll(),!1):o===l._async&&a(t._function)&&!Y()?(m(),_e(r,!1),!1):e&&o===l._async&&!Y()&&a(t.name)?(s.scopeDepth++,he(!1),d(t.arrow),ht(n),!0):a(t._do)&&!Y()?(m(),Ie(),!1):e&&!Y()&&a(t.arrow)?(s.scopeDepth++,dn(!1),d(t.arrow),ht(n),!0):(s.tokens[s.tokens.length-1].identifierRole=_.Access,!1)}case t._do:return m(),Ie(),!1;case t.parenL:return Ui(e);case t.bracketL:return m(),Hi(t.bracketR,!0),!1;case t.braceL:return Pt(!1,!1),!1;case t._function:return Il(),!1;case t.at:In();case t._class:return ve(!1),!1;case t._new:return El(),!1;case t.backQuote:return xn(),!1;case t.doubleColon:return m(),Ws(),!1;case t.hash:{let n=is();return be[n]||n===u.backslash?_n():m(),!1}default:return S(),!1}}function _n(){f(t.hash),I()}function Il(){let e=s.start;I(),f(t.dot)&&I(),_e(e,!1)}function Pe(){m()}function Dt(){d(t.parenL),X(),d(t.parenR)}function Ui(e){let n=s.snapshot(),r=s.tokens.length;d(t.parenL);let o=!0;for(;!a(t.parenR)&&!s.error;){if(o)o=!1;else if(d(t.comma),a(t.parenR))break;if(a(t.ellipsis)){Ss(!1),zs();break}else z(!1,!0)}return d(t.parenR),e&&Sl()&&wn()?(s.restoreFromSnapshot(n),s.scopeDepth++,we(),wn(),ht(r),s.error?(s.restoreFromSnapshot(n),Ui(!1),!1):!0):!1}function Sl(){return a(t.colon)||!Y()}function wn(){return R?Oi():L?Yi():f(t.arrow)}function zs(){(R||L)&&$i()}function El(){if(d(t._new),f(t.dot)){I();return}Al(),L&&Xi(),f(t.parenL)&&Hi(t.parenR)}function Al(){Ws(),f(t.questionDot)}function xn(){for(me(),me();!a(t.backQuote)&&!s.error;)d(t.dollarBraceL),X(),me(),me();m()}function Pt(e,n){let r=Ne(),o=!0;for(m(),s.tokens[s.tokens.length-1].contextId=r;!f(t.braceR)&&!s.error;){if(o)o=!1;else if(d(t.comma),f(t.braceR))break;let i=!1;if(a(t.ellipsis)){let c=s.tokens.length;if(Is(),e&&(s.tokens.length===c+2&&dn(n),f(t.braceR)))break;continue}e||(i=f(t.star)),!e&&b(l._async)?(i&&S(),I(),a(t.colon)||a(t.parenL)||a(t.braceR)||a(t.eq)||a(t.comma)||(a(t.star)&&(m(),i=!0),De(r))):De(r),Rl(e,n,r)}s.tokens[s.tokens.length-1].contextId=r}function Pl(e){return!e&&(a(t.string)||a(t.num)||a(t.bracketL)||a(t.name)||!!(s.type&t.IS_KEYWORD))}function Cl(e,n){let r=s.start;return a(t.parenL)?(e&&S(),vn(r,!1),!0):Pl(e)?(De(n),vn(r,!1),!0):!1}function Nl(e,n){if(f(t.colon)){e?It(n):z(!1);return}let r;e?s.scopeDepth===0?r=_.ObjectShorthandTopLevelDeclaration:n?r=_.ObjectShorthandBlockScopedDeclaration:r=_.ObjectShorthandFunctionScopedDeclaration:r=_.ObjectShorthand,s.tokens[s.tokens.length-1].identifierRole=r,It(n,!0)}function Rl(e,n,r){R?Ci():L&&Ji(),Cl(e,r)||Nl(e,n)}function De(e){L&&Tn(),f(t.bracketL)?(s.tokens[s.tokens.length-1].contextId=e,z(),d(t.bracketR),s.tokens[s.tokens.length-1].contextId=e):(a(t.num)||a(t.string)||a(t.bigint)||a(t.decimal)?pe():_n(),s.tokens[s.tokens.length-1].identifierRole=_.ObjectKey,s.tokens[s.tokens.length-1].contextId=e)}function vn(e,n){let r=Ne();s.scopeDepth++;let o=s.tokens.length;we(n,r),Xs(e,r);let c=s.tokens.length;s.scopes.push(new te(o,c,!0)),s.scopeDepth--}function ht(e){Oe(!0);let n=s.tokens.length;s.scopes.push(new te(e,n,!0)),s.scopeDepth--}function Xs(e,n=0){R?xi(e,n):L?Wi(n):Oe(!1,n)}function Oe(e,n=0){e&&!a(t.braceL)?z():Ie(!0,n)}function Hi(e,n=!1){let r=!0;for(;!f(e)&&!s.error;){if(r)r=!1;else if(d(t.comma),f(e))break;Vi(n)}}function Vi(e){e&&a(t.comma)||(a(t.ellipsis)?(Is(),zs()):a(t.question)?m():z(!1,!0))}function I(){m(),s.tokens[s.tokens.length-1].type=t.name}function Ll(){ut()}function Dl(){m(),!a(t.semi)&&!Y()&&(f(t.star),z())}function Ol(){U(l._module),d(t.braceL),pt(t.braceR)}function Ml(e){return(e.type===t.name||!!(e.type&t.IS_KEYWORD))&&e.contextualKeyword!==l._from}function ye(e){let n=C(0);d(e||t.colon),re(),P(n)}function Zi(){d(t.modulo),U(l._checks),f(t.parenL)&&(X(),d(t.parenR))}function Ks(){let e=C(0);d(t.colon),a(t.modulo)?Zi():(re(),a(t.modulo)&&Zi()),P(e)}function jl(){m(),Ys(!0)}function Fl(){m(),I(),a(t.lessThan)&&fe(),d(t.parenL),Gs(),d(t.parenR),Ks(),F()}function Js(){a(t._class)?jl():a(t._function)?Fl():a(t._var)?Bl():W(l._module)?f(t.dot)?Ul():ql():b(l._type)?Hl():b(l._opaque)?Vl():b(l._interface)?Wl():a(t._export)?$l():S()}function Bl(){m(),o1(),F()}function ql(){for(a(t.string)?pe():I(),d(t.braceL);!a(t.braceR)&&!s.error;)a(t._import)?(m(),so()):S();d(t.braceR)}function $l(){d(t._export),f(t._default)?a(t._function)||a(t._class)?Js():(re(),F()):a(t._var)||a(t._function)||a(t._class)||b(l._opaque)?Js():a(t.star)||a(t.braceL)||b(l._interface)||b(l._type)||b(l._opaque)?ro():S()}function Ul(){U(l._exports),Te(),F()}function Hl(){m(),Zs()}function Vl(){m(),eo(!0)}function Wl(){m(),Ys()}function Ys(e=!1){if(Cn(),a(t.lessThan)&&fe(),f(t._extends))do Sn();while(!e&&f(t.comma));if(b(l._mixins)){m();do Sn();while(f(t.comma))}if(b(l._implements)){m();do Sn();while(f(t.comma))}En(e,!1,e)}function Sn(){n1(!1),a(t.lessThan)&&Me()}function Qs(){Ys()}function Cn(){I()}function Zs(){Cn(),a(t.lessThan)&&fe(),ye(t.eq),F()}function eo(e){U(l._type),Cn(),a(t.lessThan)&&fe(),a(t.colon)&&ye(t.colon),e||ye(t.eq),F()}function zl(){Tn(),o1(),f(t.eq)&&re()}function fe(){let e=C(0);a(t.lessThan)||a(t.typeParameterStart)?m():S();do zl(),a(t.greaterThan)||d(t.comma);while(!a(t.greaterThan)&&!s.error);d(t.greaterThan),P(e)}function Me(){let e=C(0);for(d(t.lessThan);!a(t.greaterThan)&&!s.error;)re(),a(t.greaterThan)||d(t.comma);d(t.greaterThan),P(e)}function Xl(){if(U(l._interface),f(t._extends))do Sn();while(f(t.comma));En(!1,!1,!1)}function to(){a(t.num)||a(t.string)?pe():I()}function Jl(){B()===t.colon?(to(),ye()):re(),d(t.bracketR),ye()}function Gl(){to(),d(t.bracketR),d(t.bracketR),a(t.lessThan)||a(t.parenL)?no():(f(t.question),ye())}function no(){for(a(t.lessThan)&&fe(),d(t.parenL);!a(t.parenR)&&!a(t.ellipsis)&&!s.error;)An(),a(t.parenR)||d(t.comma);f(t.ellipsis)&&An(),d(t.parenR),ye()}function Kl(){no()}function En(e,n,r){let o;for(n&&a(t.braceBarL)?(d(t.braceBarL),o=t.braceBarR):(d(t.braceL),o=t.braceR);!a(o)&&!s.error;){if(r&&b(l._proto)){let i=B();i!==t.colon&&i!==t.question&&(m(),e=!1)}if(e&&b(l._static)){let i=B();i!==t.colon&&i!==t.question&&m()}if(Tn(),f(t.bracketL))f(t.bracketL)?Gl():Jl();else if(a(t.parenL)||a(t.lessThan))Kl();else{if(b(l._get)||b(l._set)){let i=B();(i===t.name||i===t.string||i===t.num)&&m()}Yl()}Ql()}d(o)}function Yl(){if(a(t.ellipsis)){if(d(t.ellipsis),f(t.comma)||f(t.semi),a(t.braceR))return;re()}else to(),a(t.lessThan)||a(t.parenL)?no():(f(t.question),ye())}function Ql(){!f(t.semi)&&!f(t.comma)&&!a(t.braceR)&&!a(t.braceBarR)&&S()}function n1(e){for(e||I();f(t.dot);)I()}function Zl(){n1(!0),a(t.lessThan)&&Me()}function eu(){d(t._typeof),r1()}function tu(){for(d(t.bracketL);s.pos<k.length&&!a(t.bracketR)&&(re(),!a(t.bracketR));)d(t.comma);d(t.bracketR)}function An(){let e=B();e===t.colon||e===t.question?(I(),f(t.question),ye()):re()}function Gs(){for(;!a(t.parenR)&&!a(t.ellipsis)&&!s.error;)An(),a(t.parenR)||d(t.comma);f(t.ellipsis)&&An()}function r1(){let e=!1,n=s.noAnonFunctionType;switch(s.type){case t.name:{if(b(l._interface)){Xl();return}I(),Zl();return}case t.braceL:En(!1,!1,!1);return;case t.braceBarL:En(!1,!0,!1);return;case t.bracketL:tu();return;case t.lessThan:fe(),d(t.parenL),Gs(),d(t.parenR),d(t.arrow),re();return;case t.parenL:if(m(),!a(t.parenR)&&!a(t.ellipsis))if(a(t.name)){let r=B();e=r!==t.question&&r!==t.colon}else e=!0;if(e)if(s.noAnonFunctionType=!1,re(),s.noAnonFunctionType=n,s.noAnonFunctionType||!(a(t.comma)||a(t.parenR)&&B()===t.arrow)){d(t.parenR);return}else f(t.comma);Gs(),d(t.parenR),d(t.arrow),re();return;case t.minus:m(),Pe();return;case t.string:case t.num:case t._true:case t._false:case t._null:case t._this:case t._void:case t.star:m();return;default:if(s.type===t._typeof){eu();return}else if(s.type&t.IS_KEYWORD){m(),s.tokens[s.tokens.length-1].type=t.name;return}}S()}function nu(){for(r1();!Y()&&(a(t.bracketL)||a(t.questionDot));)f(t.questionDot),d(t.bracketL),f(t.bracketR)||(re(),d(t.bracketR))}function s1(){f(t.question)?s1():nu()}function e1(){s1(),!s.noAnonFunctionType&&f(t.arrow)&&re()}function t1(){for(f(t.bitwiseAND),e1();f(t.bitwiseAND);)e1()}function ru(){for(f(t.bitwiseOR),t1();f(t.bitwiseOR);)t1()}function re(){ru()}function Te(){ye()}function o1(){I(),a(t.colon)&&Te()}function Tn(){(a(t.plus)||a(t.minus))&&(m(),s.tokens[s.tokens.length-1].isType=!0)}function Wi(e){a(t.colon)&&Ks(),Oe(!1,e)}function zi(e,n,r){if(a(t.questionDot)&&B()===t.lessThan){if(n){r.stop=!0;return}m(),Me(),d(t.parenL),de();return}else if(!n&&a(t.lessThan)){let o=s.snapshot();if(Me(),d(t.parenL),de(),s.error)s.restoreFromSnapshot(o);else return}Rt(e,n,r)}function Xi(){if(a(t.lessThan)){let e=s.snapshot();Me(),s.error&&s.restoreFromSnapshot(e)}}function i1(){if(a(t.name)&&s.contextualKeyword===l._interface){let e=C(0);return m(),Qs(),P(e),!0}else if(b(l._enum))return x1(),!0;return!1}function a1(){return b(l._enum)?(x1(),!0):!1}function c1(e){if(e===l._declare){if(a(t._class)||a(t.name)||a(t._function)||a(t._var)||a(t._export)){let n=C(1);Js(),P(n)}}else if(a(t.name)){if(e===l._interface){let n=C(1);Qs(),P(n)}else if(e===l._type){let n=C(1);Zs(),P(n)}else if(e===l._opaque){let n=C(1);eo(!1),P(n)}}F()}function l1(){return b(l._type)||b(l._interface)||b(l._opaque)||b(l._enum)}function u1(){return a(t.name)&&(s.contextualKeyword===l._type||s.contextualKeyword===l._interface||s.contextualKeyword===l._opaque||s.contextualKeyword===l._enum)}function p1(){if(b(l._type)){let e=C(1);m(),a(t.braceL)?(Nn(),dt()):Zs(),P(e)}else if(b(l._opaque)){let e=C(1);m(),eo(!1),P(e)}else if(b(l._interface)){let e=C(1);m(),Qs(),P(e)}else ee(!0)}function f1(){return a(t.star)||b(l._type)&&B()===t.star}function h1(){if(W(l._type)){let e=C(2);Pn(),P(e)}else Pn()}function d1(e){if(e&&a(t.lessThan)&&Me(),b(l._implements)){let n=C(0);m(),s.tokens[s.tokens.length-1].type=t._implements;do Cn(),a(t.lessThan)&&Me();while(f(t.comma));P(n)}}function Ji(){a(t.lessThan)&&(fe(),a(t.parenL)||S())}function si(){let e=C(0);f(t.question),a(t.colon)&&Te(),P(e)}function m1(){if(a(t._typeof)||b(l._type)){let e=xe();(Ml(e)||e.type===t.braceL||e.type===t.star)&&m()}}function g1(){let e=s.contextualKeyword===l._type||s.type===t._typeof;e?m():I(),b(l._as)&&!st(l._as)?(I(),e&&!a(t.name)&&!(s.type&t.IS_KEYWORD)||I()):(e&&(a(t.name)||s.type&t.IS_KEYWORD)&&I(),W(l._as)&&I())}function k1(){if(a(t.lessThan)){let e=C(0);fe(),P(e)}}function y1(){a(t.colon)&&Te()}function Gi(){if(a(t.colon)){let e=s.noAnonFunctionType;s.noAnonFunctionType=!0,Te(),s.noAnonFunctionType=e}}function Ki(e,n){if(a(t.lessThan)){let r=s.snapshot(),o=ue(e,n);if(s.error)s.restoreFromSnapshot(r),s.type=t.typeParameterStart;else return o;let i=C(0);if(fe(),P(i),o=ue(e,n),o)return!0;S()}return ue(e,n)}function Yi(){if(a(t.colon)){let e=C(0),n=s.snapshot(),r=s.noAnonFunctionType;s.noAnonFunctionType=!0,Ks(),s.noAnonFunctionType=r,Y()&&S(),a(t.arrow)||S(),s.error&&s.restoreFromSnapshot(n),P(e)}return f(t.arrow)}function Qi(e,n=!1){if(s.tokens[s.tokens.length-1].contextualKeyword===l._async&&a(t.lessThan)){let r=s.snapshot();if(su()&&!s.error)return;s.restoreFromSnapshot(r)}Vs(e,n)}function su(){s.scopeDepth++;let e=s.tokens.length;return we(),wn()?(ht(e),!0):!1}function x1(){U(l._enum),s.tokens[s.tokens.length-1].type=t._enum,I(),ou()}function ou(){W(l._of)&&m(),d(t.braceL),iu(),d(t.braceR)}function iu(){for(;!a(t.braceR)&&!s.error&&!f(t.ellipsis);)au(),a(t.braceR)||d(t.comma)}function au(){I(),f(t.eq)&&m()}function v1(){if(pt(t.eof),s.scopes.push(new te(0,s.tokens.length,!0)),s.scopeDepth!==0)throw new Error(`Invalid scope depth at end of file: ${s.scopeDepth}`);return new Ln(s.tokens,s.scopes)}function ee(e){L&&i1()||(a(t.at)&&In(),cu(e))}function cu(e){if(R&&Ii())return;let n=s.type;switch(n){case t._break:case t._continue:uu();return;case t._debugger:pu();return;case t._do:fu();return;case t._for:hu();return;case t._function:if(B()===t.dot)break;e||S(),gu();return;case t._class:e||S(),ve(!0);return;case t._if:ku();return;case t._return:yu();return;case t._switch:xu();return;case t._throw:bu();return;case t._try:wu();return;case t._let:case t._const:e||S();case t._var:Lt(n!==t._var);return;case t._while:vu();return;case t.braceL:Ie();return;case t.semi:Tu();return;case t._export:case t._import:{let i=B();if(i===t.parenL||i===t.dot)break;m(),n===t._import?so():ro();return}case t.name:if(s.contextualKeyword===l._async){let i=s.start,c=s.snapshot();if(m(),a(t._function)&&!Y()){d(t._function),_e(i,!0);return}else s.restoreFromSnapshot(c)}else if(s.contextualKeyword===l._using&&!tn()&&B()===t.name){Lt(!0);return}else if(T1()){U(l._await),Lt(!0);return}default:break}let r=s.tokens.length;X();let o=null;if(s.tokens.length===r+1){let i=s.tokens[s.tokens.length-1];i.type===t.name&&(o=i.contextualKeyword)}if(o==null){F();return}f(t.colon)?Iu():Su(o)}function T1(){if(!b(l._await))return!1;let e=s.snapshot();return m(),!b(l._using)||K()?(s.restoreFromSnapshot(e),!1):(m(),!a(t.name)||K()?(s.restoreFromSnapshot(e),!1):(s.restoreFromSnapshot(e),!0))}function In(){for(;a(t.at);)I1()}function I1(){if(m(),f(t.parenL))X(),d(t.parenR);else{for(I();f(t.dot);)I();lu()}}function lu(){R?Mi():js()}function js(){f(t.parenL)&&de()}function uu(){m(),le()||(I(),F())}function pu(){m(),F()}function fu(){m(),ee(!1),d(t._while),Dt(),f(t.semi)}function hu(){s.scopeDepth++;let e=s.tokens.length;mu();let n=s.tokens.length;s.scopes.push(new te(e,n,!1)),s.scopeDepth--}function du(){return!(!b(l._using)||st(l._of))}function mu(){m();let e=!1;if(b(l._await)&&(e=!0,m()),d(t.parenL),a(t.semi)){e&&S(),oo();return}let n=T1();if(n||a(t._var)||a(t._let)||a(t._const)||du()){if(n&&U(l._await),m(),S1(!0,s.type!==t._var),a(t._in)||b(l._of)){b1(e);return}oo();return}if(X(!0),a(t._in)||b(l._of)){b1(e);return}e&&S(),oo()}function gu(){let e=s.start;m(),_e(e,!0)}function ku(){m(),Dt(),ee(!1),f(t._else)&&ee(!1)}function yu(){m(),le()||(X(),F())}function xu(){m(),Dt(),s.scopeDepth++;let e=s.tokens.length;for(d(t.braceL);!a(t.braceR)&&!s.error;)if(a(t._case)||a(t._default)){let r=a(t._case);m(),r&&X(),d(t.colon)}else ee(!0);m();let n=s.tokens.length;s.scopes.push(new te(e,n,!1)),s.scopeDepth--}function bu(){m(),X(),F()}function _u(){St(!0),R&&Le()}function wu(){if(m(),Ie(),a(t._catch)){m();let e=null;if(a(t.parenL)&&(s.scopeDepth++,e=s.tokens.length,d(t.parenL),_u(),d(t.parenR)),Ie(),e!=null){let n=s.tokens.length;s.scopes.push(new te(e,n,!1)),s.scopeDepth--}}f(t._finally)&&Ie()}function Lt(e){m(),S1(!1,e),F()}function vu(){m(),Dt(),ee(!1)}function Tu(){m()}function Iu(){ee(!0)}function Su(e){R?Ei(e):L?c1(e):F()}function Ie(e=!1,n=0){let r=s.tokens.length;s.scopeDepth++,d(t.braceL),n&&(s.tokens[s.tokens.length-1].contextId=n),pt(t.braceR),n&&(s.tokens[s.tokens.length-1].contextId=n);let o=s.tokens.length;s.scopes.push(new te(r,o,e)),s.scopeDepth--}function pt(e){for(;!f(e)&&!s.error;)ee(!0)}function oo(){d(t.semi),a(t.semi)||X(),d(t.semi),a(t.parenR)||X(),d(t.parenR),ee(!1)}function b1(e){e?W(l._of):m(),X(),d(t.parenR),ee(!1)}function S1(e,n){for(;;){if(Eu(n),f(t.eq)){let r=s.tokens.length-1;z(e),s.tokens[r].rhsEndIndex=s.tokens.length}if(!f(t.comma))break}}function Eu(e){St(e),R?Ri():L&&y1()}function _e(e,n,r=!1){a(t.star)&&m(),n&&!r&&!a(t.name)&&!a(t._yield)&&S();let o=null;a(t.name)&&(n||(o=s.tokens.length,s.scopeDepth++),he(!1));let i=s.tokens.length;s.scopeDepth++,we(),Xs(e);let c=s.tokens.length;s.scopes.push(new te(i,c,!0)),s.scopeDepth--,o!==null&&(s.scopes.push(new te(o,c,!0)),s.scopeDepth--)}function we(e=!1,n=0){R?Ni():L&&k1(),d(t.parenL),n&&(s.tokens[s.tokens.length-1].contextId=n),Et(t.parenR,!1,!1,e,n),n&&(s.tokens[s.tokens.length-1].contextId=n)}function ve(e,n=!1){let r=Ne();m(),s.tokens[s.tokens.length-1].contextId=r,s.tokens[s.tokens.length-1].isExpression=!e;let o=null;e||(o=s.tokens.length,s.scopeDepth++),Nu(e,n),Ru();let i=s.tokens.length;if(Au(r),!s.error&&(s.tokens[i].contextId=r,s.tokens[s.tokens.length-1].contextId=r,o!==null)){let c=s.tokens.length;s.scopes.push(new te(o,c,!1)),s.scopeDepth--}}function E1(){return a(t.eq)||a(t.semi)||a(t.braceR)||a(t.bang)||a(t.colon)}function A1(){return a(t.parenL)||a(t.lessThan)}function Au(e){for(d(t.braceL);!f(t.braceR)&&!s.error;){if(f(t.semi))continue;if(a(t.at)){I1();continue}let n=s.start;Pu(n,e)}}function Pu(e,n){R&&At([l._declare,l._public,l._protected,l._private,l._override]);let r=!1;if(a(t.name)&&s.contextualKeyword===l._static){if(I(),A1()){Mt(e,!1);return}else if(E1()){Rn();return}if(s.tokens[s.tokens.length-1].type=t._static,r=!0,a(t.braceL)){s.tokens[s.tokens.length-1].contextId=n,Ie();return}}Cu(e,r,n)}function Cu(e,n,r){if(R&&Si(n))return;if(f(t.star)){Ot(r),Mt(e,!1);return}Ot(r);let o=!1,i=s.tokens[s.tokens.length-1];i.contextualKeyword===l._constructor&&(o=!0),_1(),A1()?Mt(e,o):E1()?Rn():i.contextualKeyword===l._async&&!le()?(s.tokens[s.tokens.length-1].type=t._async,a(t.star)&&m(),Ot(r),_1(),Mt(e,!1)):(i.contextualKeyword===l._get||i.contextualKeyword===l._set)&&!(le()&&a(t.star))?(i.contextualKeyword===l._get?s.tokens[s.tokens.length-1].type=t._get:s.tokens[s.tokens.length-1].type=t._set,Ot(r),Mt(e,!1)):i.contextualKeyword===l._accessor&&!le()?(Ot(r),Rn()):le()?Rn():S()}function Mt(e,n){R?Ce():L&&a(t.lessThan)&&fe(),vn(e,n)}function Ot(e){De(e)}function _1(){if(R){let e=C(0);f(t.question),P(e)}}function Rn(){if(R?(sn(t.bang),Le()):L&&a(t.colon)&&Te(),a(t.eq)){let e=s.tokens.length;m(),z(),s.tokens[e].rhsEndIndex=s.tokens.length}F()}function Nu(e,n=!1){R&&(!e||n)&&b(l._implements)||(a(t.name)&&he(!0),R?Ce():L&&a(t.lessThan)&&fe())}function Ru(){let e=!1;f(t._extends)?(Us(),e=!0):e=!1,R?Pi(e):L&&d1(e)}function ro(){let e=s.tokens.length-1;R&&_i()||(Mu()?ju():Ou()?(I(),a(t.comma)&&B()===t.star?(d(t.comma),d(t.star),U(l._as),I()):P1(),dt()):f(t._default)?Lu():Bu()?Du():(Nn(),dt()),s.tokens[e].rhsEndIndex=s.tokens.length)}function Lu(){if(R&&Ti()||L&&a1())return;let e=s.start;f(t._function)?_e(e,!0,!0):b(l._async)&&B()===t._function?(W(l._async),f(t._function),_e(e,!0,!0)):a(t._class)?ve(!0,!0):a(t.at)?(In(),ve(!0,!0)):(z(),F())}function Du(){R?Ai():L?p1():ee(!0)}function Ou(){if(R&&Os())return!1;if(L&&u1())return!1;if(a(t.name))return s.contextualKeyword!==l._async;if(!a(t._default))return!1;let e=kt(),n=xe(),r=n.type===t.name&&n.contextualKeyword===l._from;if(n.type===t.comma)return!0;if(r){let o=k.charCodeAt(os(e+4));return o===u.quotationMark||o===u.apostrophe}return!1}function P1(){f(t.comma)&&Nn()}function dt(){W(l._from)&&(pe(),C1()),F()}function Mu(){return L?f1():a(t.star)}function ju(){L?h1():Pn()}function Pn(){d(t.star),b(l._as)?Fu():dt()}function Fu(){m(),s.tokens[s.tokens.length-1].type=t._as,I(),P1(),dt()}function Bu(){return R&&Os()||L&&l1()||s.type===t._var||s.type===t._const||s.type===t._let||s.type===t._function||s.type===t._class||b(l._async)||a(t.at)}function Nn(){let e=!0;for(d(t.braceL);!f(t.braceR)&&!s.error;){if(e)e=!1;else if(d(t.comma),f(t.braceR))break;qu()}}function qu(){if(R){vi();return}I(),s.tokens[s.tokens.length-1].identifierRole=_.ExportAccess,W(l._as)&&I()}function $u(){let e=s.snapshot();return U(l._module),W(l._from)?b(l._from)?(s.restoreFromSnapshot(e),!0):(s.restoreFromSnapshot(e),!1):a(t.comma)?(s.restoreFromSnapshot(e),!1):(s.restoreFromSnapshot(e),!0)}function Uu(){b(l._module)&&$u()&&m()}function so(){if(R&&a(t.name)&&B()===t.eq){kn();return}if(R&&b(l._type)){let e=xe();if(e.type===t.name&&e.contextualKeyword!==l._from){if(U(l._type),B()===t.eq){kn();return}}else(e.type===t.star||e.type===t.braceL)&&U(l._type)}a(t.string)?pe():(Uu(),Vu(),U(l._from),pe()),C1(),F()}function Hu(){return a(t.name)}function w1(){it()}function Vu(){L&&m1();let e=!0;if(!(Hu()&&(w1(),!f(t.comma)))){if(a(t.star)){m(),U(l._as),w1();return}for(d(t.braceL);!f(t.braceR)&&!s.error;){if(e)e=!1;else if(f(t.colon)&&S("ES2015 named imports do not destructure. Use another statement for destructuring after the import."),d(t.comma),f(t.braceR))break;Wu()}}}function Wu(){if(R){wi();return}if(L){g1();return}it(),b(l._as)&&(s.tokens[s.tokens.length-1].identifierRole=_.ImportAccess,m(),it())}function C1(){(a(t._with)||b(l._assert)&&!K())&&(m(),Pt(!1,!1))}function N1(){return s.pos===0&&k.charCodeAt(0)===u.numberSign&&k.charCodeAt(1)===u.exclamationMark&&cs(2),as(),v1()}var Ln=class{constructor(n,r){this.tokens=n,this.scopes=r}};function R1(e,n,r,o){if(o&&r)throw new Error("Cannot combine flow and typescript plugins.");mo(e,n,r,o);let i=N1();if(s.error)throw ho(s.error);return i}function io(e){let n=e.currentIndex(),r=0,o=e.currentToken();do{let i=e.tokens[n];if(i.isOptionalChainStart&&r++,i.isOptionalChainEnd&&r--,r+=i.numNullishCoalesceStarts,r-=i.numNullishCoalesceEnds,i.contextualKeyword===l._await&&i.identifierRole==null&&i.scopeDepth===o.scopeDepth)return!0;n+=1}while(r>0&&n<e.tokens.length);return!1}var jt=class e{__init(){this.resultCode=""}__init2(){this.resultMappings=new Array(this.tokens.length)}__init3(){this.tokenIndex=0}constructor(n,r,o,i,c){this.code=n,this.tokens=r,this.isFlowEnabled=o,this.disableESTransforms=i,this.helperManager=c,e.prototype.__init.call(this),e.prototype.__init2.call(this),e.prototype.__init3.call(this)}snapshot(){return{resultCode:this.resultCode,tokenIndex:this.tokenIndex}}restoreToSnapshot(n){this.resultCode=n.resultCode,this.tokenIndex=n.tokenIndex}dangerouslyGetAndRemoveCodeSinceSnapshot(n){let r=this.resultCode.slice(n.resultCode.length);return this.resultCode=n.resultCode,r}reset(){this.resultCode="",this.resultMappings=new Array(this.tokens.length),this.tokenIndex=0}matchesContextualAtIndex(n,r){return this.matches1AtIndex(n,t.name)&&this.tokens[n].contextualKeyword===r}identifierNameAtIndex(n){return this.identifierNameForToken(this.tokens[n])}identifierNameAtRelativeIndex(n){return this.identifierNameForToken(this.tokenAtRelativeIndex(n))}identifierName(){return this.identifierNameForToken(this.currentToken())}identifierNameForToken(n){return this.code.slice(n.start,n.end)}rawCodeForToken(n){return this.code.slice(n.start,n.end)}stringValueAtIndex(n){return this.stringValueForToken(this.tokens[n])}stringValue(){return this.stringValueForToken(this.currentToken())}stringValueForToken(n){return this.code.slice(n.start+1,n.end-1)}matches1AtIndex(n,r){return this.tokens[n].type===r}matches2AtIndex(n,r,o){return this.tokens[n].type===r&&this.tokens[n+1].type===o}matches3AtIndex(n,r,o,i){return this.tokens[n].type===r&&this.tokens[n+1].type===o&&this.tokens[n+2].type===i}matches1(n){return this.tokens[this.tokenIndex].type===n}matches2(n,r){return this.tokens[this.tokenIndex].type===n&&this.tokens[this.tokenIndex+1].type===r}matches3(n,r,o){return this.tokens[this.tokenIndex].type===n&&this.tokens[this.tokenIndex+1].type===r&&this.tokens[this.tokenIndex+2].type===o}matches4(n,r,o,i){return this.tokens[this.tokenIndex].type===n&&this.tokens[this.tokenIndex+1].type===r&&this.tokens[this.tokenIndex+2].type===o&&this.tokens[this.tokenIndex+3].type===i}matches5(n,r,o,i,c){return this.tokens[this.tokenIndex].type===n&&this.tokens[this.tokenIndex+1].type===r&&this.tokens[this.tokenIndex+2].type===o&&this.tokens[this.tokenIndex+3].type===i&&this.tokens[this.tokenIndex+4].type===c}matchesContextual(n){return this.matchesContextualAtIndex(this.tokenIndex,n)}matchesContextIdAndLabel(n,r){return this.matches1(n)&&this.currentToken().contextId===r}previousWhitespaceAndComments(){let n=this.code.slice(this.tokenIndex>0?this.tokens[this.tokenIndex-1].end:0,this.tokenIndex<this.tokens.length?this.tokens[this.tokenIndex].start:this.code.length);return this.isFlowEnabled&&(n=n.replace(/@flow/g,"")),n}replaceToken(n){this.resultCode+=this.previousWhitespaceAndComments(),this.appendTokenPrefix(),this.resultMappings[this.tokenIndex]=this.resultCode.length,this.resultCode+=n,this.appendTokenSuffix(),this.tokenIndex++}replaceTokenTrimmingLeftWhitespace(n){this.resultCode+=this.previousWhitespaceAndComments().replace(/[^\r\n]/g,""),this.appendTokenPrefix(),this.resultMappings[this.tokenIndex]=this.resultCode.length,this.resultCode+=n,this.appendTokenSuffix(),this.tokenIndex++}removeInitialToken(){this.replaceToken("")}removeToken(){this.replaceTokenTrimmingLeftWhitespace("")}removeBalancedCode(){let n=0;for(;!this.isAtEnd();){if(this.matches1(t.braceL))n++;else if(this.matches1(t.braceR)){if(n===0)return;n--}this.removeToken()}}copyExpectedToken(n){if(this.tokens[this.tokenIndex].type!==n)throw new Error(`Expected token ${n}`);this.copyToken()}copyToken(){this.resultCode+=this.previousWhitespaceAndComments(),this.appendTokenPrefix(),this.resultMappings[this.tokenIndex]=this.resultCode.length,this.resultCode+=this.code.slice(this.tokens[this.tokenIndex].start,this.tokens[this.tokenIndex].end),this.appendTokenSuffix(),this.tokenIndex++}copyTokenWithPrefix(n){this.resultCode+=this.previousWhitespaceAndComments(),this.appendTokenPrefix(),this.resultCode+=n,this.resultMappings[this.tokenIndex]=this.resultCode.length,this.resultCode+=this.code.slice(this.tokens[this.tokenIndex].start,this.tokens[this.tokenIndex].end),this.appendTokenSuffix(),this.tokenIndex++}appendTokenPrefix(){let n=this.currentToken();if((n.numNullishCoalesceStarts||n.isOptionalChainStart)&&(n.isAsyncOperation=io(this)),!this.disableESTransforms){if(n.numNullishCoalesceStarts)for(let r=0;r<n.numNullishCoalesceStarts;r++)n.isAsyncOperation?(this.resultCode+="await ",this.resultCode+=this.helperManager.getHelperName("asyncNullishCoalesce")):this.resultCode+=this.helperManager.getHelperName("nullishCoalesce"),this.resultCode+="(";n.isOptionalChainStart&&(n.isAsyncOperation&&(this.resultCode+="await "),this.tokenIndex>0&&this.tokenAtRelativeIndex(-1).type===t._delete?n.isAsyncOperation?this.resultCode+=this.helperManager.getHelperName("asyncOptionalChainDelete"):this.resultCode+=this.helperManager.getHelperName("optionalChainDelete"):n.isAsyncOperation?this.resultCode+=this.helperManager.getHelperName("asyncOptionalChain"):this.resultCode+=this.helperManager.getHelperName("optionalChain"),this.resultCode+="([")}}appendTokenSuffix(){let n=this.currentToken();if(n.isOptionalChainEnd&&!this.disableESTransforms&&(this.resultCode+="])"),n.numNullishCoalesceEnds&&!this.disableESTransforms)for(let r=0;r<n.numNullishCoalesceEnds;r++)this.resultCode+="))"}appendCode(n){this.resultCode+=n}currentToken(){return this.tokens[this.tokenIndex]}currentTokenCode(){let n=this.currentToken();return this.code.slice(n.start,n.end)}tokenAtRelativeIndex(n){return this.tokens[this.tokenIndex+n]}currentIndex(){return this.tokenIndex}nextToken(){if(this.tokenIndex===this.tokens.length)throw new Error("Unexpectedly reached end of input.");this.tokenIndex++}previousToken(){this.tokenIndex--}finish(){if(this.tokenIndex!==this.tokens.length)throw new Error("Tried to finish processing tokens before reaching the end.");return this.resultCode+=this.previousWhitespaceAndComments(),{code:this.resultCode,mappings:this.resultMappings}}isAtEnd(){return this.tokenIndex===this.tokens.length}};function co(e,n,r,o){let i=n.snapshot(),c=zu(n),p=[],h=[],g=[],y=null,x=[],v=[],T=n.currentToken().contextId;if(T==null)throw new Error("Expected non-null class context ID on class open-brace.");for(n.nextToken();!n.matchesContextIdAndLabel(t.braceR,T);)if(n.matchesContextual(l._constructor)&&!n.currentToken().isType)({constructorInitializerStatements:p,constructorInsertPos:y}=L1(n));else if(n.matches1(t.semi))o||v.push({start:n.currentIndex(),end:n.currentIndex()+1}),n.nextToken();else if(n.currentToken().isType)n.nextToken();else{let E=n.currentIndex(),O=!1,q=!1,G=!1;for(;Dn(n.currentToken());)n.matches1(t._static)&&(O=!0),n.matches1(t.hash)&&(q=!0),(n.matches1(t._declare)||n.matches1(t._abstract))&&(G=!0),n.nextToken();if(O&&n.matches1(t.braceL)){ao(n,T);continue}if(q){ao(n,T);continue}if(n.matchesContextual(l._constructor)&&!n.currentToken().isType){({constructorInitializerStatements:p,constructorInsertPos:y}=L1(n));continue}let N=n.currentIndex();if(Xu(n),n.matches1(t.lessThan)||n.matches1(t.parenL)){ao(n,T);continue}for(;n.currentToken().isType;)n.nextToken();if(n.matches1(t.eq)){let J=n.currentIndex(),ae=n.currentToken().rhsEndIndex;if(ae==null)throw new Error("Expected rhsEndIndex on class field assignment.");for(n.nextToken();n.currentIndex()<ae;)e.processToken();let ce;O?(ce=r.claimFreeName("__initStatic"),g.push(ce)):(ce=r.claimFreeName("__init"),h.push(ce)),x.push({initializerName:ce,equalsIndex:J,start:N,end:n.currentIndex()})}else(!o||G)&&v.push({start:E,end:n.currentIndex()})}return n.restoreToSnapshot(i),o?{headerInfo:c,constructorInitializerStatements:p,instanceInitializerNames:[],staticInitializerNames:[],constructorInsertPos:y,fields:[],rangesToRemove:v}:{headerInfo:c,constructorInitializerStatements:p,instanceInitializerNames:h,staticInitializerNames:g,constructorInsertPos:y,fields:x,rangesToRemove:v}}function ao(e,n){for(e.nextToken();e.currentToken().contextId!==n;)e.nextToken();for(;Dn(e.tokenAtRelativeIndex(-1));)e.previousToken()}function zu(e){let n=e.currentToken(),r=n.contextId;if(r==null)throw new Error("Expected context ID on class token.");let o=n.isExpression;if(o==null)throw new Error("Expected isExpression on class token.");let i=null,c=!1;for(e.nextToken(),e.matches1(t.name)&&(i=e.identifierName());!e.matchesContextIdAndLabel(t.braceL,r);)e.matches1(t._extends)&&!e.currentToken().isType&&(c=!0),e.nextToken();return{isExpression:o,className:i,hasSuperclass:c}}function L1(e){let n=[];e.nextToken();let r=e.currentToken().contextId;if(r==null)throw new Error("Expected context ID on open-paren starting constructor params.");for(;!e.matchesContextIdAndLabel(t.parenR,r);)if(e.currentToken().contextId===r){if(e.nextToken(),Dn(e.currentToken())){for(e.nextToken();Dn(e.currentToken());)e.nextToken();let c=e.currentToken();if(c.type!==t.name)throw new Error("Expected identifier after access modifiers in constructor arg.");let p=e.identifierNameForToken(c);n.push(`this.${p} = ${p}`)}}else e.nextToken();for(e.nextToken();e.currentToken().isType;)e.nextToken();let o=e.currentIndex(),i=!1;for(;!e.matchesContextIdAndLabel(t.braceR,r);){if(!i&&e.matches2(t._super,t.parenL)){e.nextToken();let c=e.currentToken().contextId;if(c==null)throw new Error("Expected a context ID on the super call");for(;!e.matchesContextIdAndLabel(t.parenR,c);)e.nextToken();o=e.currentIndex(),i=!0}e.nextToken()}return e.nextToken(),{constructorInitializerStatements:n,constructorInsertPos:o}}function Dn(e){return[t._async,t._get,t._set,t.plus,t.minus,t._readonly,t._static,t._public,t._private,t._protected,t._override,t._abstract,t.star,t._declare,t.hash].includes(e.type)}function Xu(e){if(e.matches1(t.bracketL)){let r=e.currentToken().contextId;if(r==null)throw new Error("Expected class context ID on computed name open bracket.");for(;!e.matchesContextIdAndLabel(t.bracketR,r);)e.nextToken();e.nextToken()}else e.nextToken()}function Ft(e){if(e.removeInitialToken(),e.removeToken(),e.removeToken(),e.removeToken(),e.matches1(t.parenL))e.removeToken(),e.removeToken(),e.removeToken();else for(;e.matches1(t.dot);)e.removeToken(),e.removeToken()}var On={typeDeclarations:new Set,valueDeclarations:new Set};function Bt(e){let n=new Set,r=new Set;for(let o=0;o<e.tokens.length;o++){let i=e.tokens[o];i.type===t.name&&rn(i)&&(i.isType?n.add(e.identifierNameForToken(i)):r.add(e.identifierNameForToken(i)))}return{typeDeclarations:n,valueDeclarations:r}}function qt(e){let n=e.currentIndex();for(;!e.matches1AtIndex(n,t.braceR);)n++;return e.matchesContextualAtIndex(n+1,l._from)&&e.matches1AtIndex(n+2,t.string)}function Se(e){(e.matches2(t._with,t.braceL)||e.matches2(t.name,t.braceL)&&e.matchesContextual(l._assert))&&(e.removeToken(),e.removeToken(),e.removeBalancedCode(),e.removeToken())}function $t(e,n,r,o){if(!e||n)return!1;let i=r.currentToken();if(i.rhsEndIndex==null)throw new Error("Expected non-null rhsEndIndex on export token.");let c=i.rhsEndIndex-r.currentIndex();if(c!==3&&!(c===4&&r.matches1AtIndex(i.rhsEndIndex-1,t.semi)))return!1;let p=r.tokenAtRelativeIndex(2);if(p.type!==t.name)return!1;let h=r.identifierNameForToken(p);return o.typeDeclarations.has(h)&&!o.valueDeclarations.has(h)}var Ut=class e extends H{__init(){this.hadExport=!1}__init2(){this.hadNamedExport=!1}__init3(){this.hadDefaultExport=!1}constructor(n,r,o,i,c,p,h,g,y,x,v,T){super(),this.rootTransformer=n,this.tokens=r,this.importProcessor=o,this.nameManager=i,this.helperManager=c,this.reactHotLoaderTransformer=p,this.enableLegacyBabel5ModuleInterop=h,this.enableLegacyTypeScriptModuleInterop=g,this.isTypeScriptTransformEnabled=y,this.isFlowTransformEnabled=x,this.preserveDynamicImport=v,this.keepUnusedImports=T,e.prototype.__init.call(this),e.prototype.__init2.call(this),e.prototype.__init3.call(this),this.declarationInfo=y?Bt(r):On}getPrefixCode(){let n="";return this.hadExport&&(n+='Object.defineProperty(exports, "__esModule", {value: true});'),n}getSuffixCode(){return this.enableLegacyBabel5ModuleInterop&&this.hadDefaultExport&&!this.hadNamedExport?`
module.exports = exports.default;
`:""}process(){return this.tokens.matches3(t._import,t.name,t.eq)?this.processImportEquals():this.tokens.matches1(t._import)?(this.processImport(),!0):this.tokens.matches2(t._export,t.eq)?(this.tokens.replaceToken("module.exports"),!0):this.tokens.matches1(t._export)&&!this.tokens.currentToken().isType?(this.hadExport=!0,this.processExport()):this.tokens.matches2(t.name,t.postIncDec)&&this.processPostIncDec()?!0:this.tokens.matches1(t.name)||this.tokens.matches1(t.jsxName)?this.processIdentifier():this.tokens.matches1(t.eq)?this.processAssignment():this.tokens.matches1(t.assign)?this.processComplexAssignment():this.tokens.matches1(t.preIncDec)?this.processPreIncDec():!1}processImportEquals(){let n=this.tokens.identifierNameAtIndex(this.tokens.currentIndex()+1);return this.importProcessor.shouldAutomaticallyElideImportedName(n)?Ft(this.tokens):this.tokens.replaceToken("const"),!0}processImport(){if(this.tokens.matches2(t._import,t.parenL)){if(this.preserveDynamicImport){this.tokens.copyToken();return}let r=this.enableLegacyTypeScriptModuleInterop?"":`${this.helperManager.getHelperName("interopRequireWildcard")}(`;this.tokens.replaceToken(`Promise.resolve().then(() => ${r}require`);let o=this.tokens.currentToken().contextId;if(o==null)throw new Error("Expected context ID on dynamic import invocation.");for(this.tokens.copyToken();!this.tokens.matchesContextIdAndLabel(t.parenR,o);)this.rootTransformer.processToken();this.tokens.replaceToken(r?")))":"))");return}if(this.removeImportAndDetectIfShouldElide())this.tokens.removeToken();else{let r=this.tokens.stringValue();this.tokens.replaceTokenTrimmingLeftWhitespace(this.importProcessor.claimImportCode(r)),this.tokens.appendCode(this.importProcessor.claimImportCode(r))}Se(this.tokens),this.tokens.matches1(t.semi)&&this.tokens.removeToken()}removeImportAndDetectIfShouldElide(){if(this.tokens.removeInitialToken(),this.tokens.matchesContextual(l._type)&&!this.tokens.matches1AtIndex(this.tokens.currentIndex()+1,t.comma)&&!this.tokens.matchesContextualAtIndex(this.tokens.currentIndex()+1,l._from))return this.removeRemainingImport(),!0;if(this.tokens.matches1(t.name)||this.tokens.matches1(t.star))return this.removeRemainingImport(),!1;if(this.tokens.matches1(t.string))return!1;let n=!1,r=!1;for(;!this.tokens.matches1(t.string);)(!n&&this.tokens.matches1(t.braceL)||this.tokens.matches1(t.comma))&&(this.tokens.removeToken(),this.tokens.matches1(t.braceR)||(r=!0),(this.tokens.matches2(t.name,t.comma)||this.tokens.matches2(t.name,t.braceR)||this.tokens.matches4(t.name,t.name,t.name,t.comma)||this.tokens.matches4(t.name,t.name,t.name,t.braceR))&&(n=!0)),this.tokens.removeToken();return this.keepUnusedImports?!1:this.isTypeScriptTransformEnabled?!n:this.isFlowTransformEnabled?r&&!n:!1}removeRemainingImport(){for(;!this.tokens.matches1(t.string);)this.tokens.removeToken()}processIdentifier(){let n=this.tokens.currentToken();if(n.shadowsGlobal)return!1;if(n.identifierRole===_.ObjectShorthand)return this.processObjectShorthand();if(n.identifierRole!==_.Access)return!1;let r=this.importProcessor.getIdentifierReplacement(this.tokens.identifierNameForToken(n));if(!r)return!1;let o=this.tokens.currentIndex()+1;for(;o<this.tokens.tokens.length&&this.tokens.tokens[o].type===t.parenR;)o++;return this.tokens.tokens[o].type===t.parenL?this.tokens.tokenAtRelativeIndex(1).type===t.parenL&&this.tokens.tokenAtRelativeIndex(-1).type!==t._new?(this.tokens.replaceToken(`${r}.call(void 0, `),this.tokens.removeToken(),this.rootTransformer.processBalancedCode(),this.tokens.copyExpectedToken(t.parenR)):this.tokens.replaceToken(`(0, ${r})`):this.tokens.replaceToken(r),!0}processObjectShorthand(){let n=this.tokens.identifierName(),r=this.importProcessor.getIdentifierReplacement(n);return r?(this.tokens.replaceToken(`${n}: ${r}`),!0):!1}processExport(){if(this.tokens.matches2(t._export,t._enum)||this.tokens.matches3(t._export,t._const,t._enum))return this.hadNamedExport=!0,!1;if(this.tokens.matches2(t._export,t._default))return this.tokens.matches3(t._export,t._default,t._enum)?(this.hadDefaultExport=!0,!1):(this.processExportDefault(),!0);if(this.tokens.matches2(t._export,t.braceL))return this.processExportBindings(),!0;if(this.tokens.matches2(t._export,t.name)&&this.tokens.matchesContextualAtIndex(this.tokens.currentIndex()+1,l._type)){if(this.tokens.removeInitialToken(),this.tokens.removeToken(),this.tokens.matches1(t.braceL)){for(;!this.tokens.matches1(t.braceR);)this.tokens.removeToken();this.tokens.removeToken()}else this.tokens.removeToken(),this.tokens.matches1(t._as)&&(this.tokens.removeToken(),this.tokens.removeToken());return this.tokens.matchesContextual(l._from)&&this.tokens.matches1AtIndex(this.tokens.currentIndex()+1,t.string)&&(this.tokens.removeToken(),this.tokens.removeToken(),Se(this.tokens)),!0}if(this.hadNamedExport=!0,this.tokens.matches2(t._export,t._var)||this.tokens.matches2(t._export,t._let)||this.tokens.matches2(t._export,t._const))return this.processExportVar(),!0;if(this.tokens.matches2(t._export,t._function)||this.tokens.matches3(t._export,t.name,t._function))return this.processExportFunction(),!0;if(this.tokens.matches2(t._export,t._class)||this.tokens.matches3(t._export,t._abstract,t._class)||this.tokens.matches2(t._export,t.at))return this.processExportClass(),!0;if(this.tokens.matches2(t._export,t.star))return this.processExportStar(),!0;throw new Error("Unrecognized export syntax.")}processAssignment(){let n=this.tokens.currentIndex(),r=this.tokens.tokens[n-1];if(r.isType||r.type!==t.name||r.shadowsGlobal||n>=2&&this.tokens.matches1AtIndex(n-2,t.dot)||n>=2&&[t._var,t._let,t._const].includes(this.tokens.tokens[n-2].type))return!1;let o=this.importProcessor.resolveExportBinding(this.tokens.identifierNameForToken(r));return o?(this.tokens.copyToken(),this.tokens.appendCode(` ${o} =`),!0):!1}processComplexAssignment(){let n=this.tokens.currentIndex(),r=this.tokens.tokens[n-1];if(r.type!==t.name||r.shadowsGlobal||n>=2&&this.tokens.matches1AtIndex(n-2,t.dot))return!1;let o=this.importProcessor.resolveExportBinding(this.tokens.identifierNameForToken(r));return o?(this.tokens.appendCode(` = ${o}`),this.tokens.copyToken(),!0):!1}processPreIncDec(){let n=this.tokens.currentIndex(),r=this.tokens.tokens[n+1];if(r.type!==t.name||r.shadowsGlobal||n+2<this.tokens.tokens.length&&(this.tokens.matches1AtIndex(n+2,t.dot)||this.tokens.matches1AtIndex(n+2,t.bracketL)||this.tokens.matches1AtIndex(n+2,t.parenL)))return!1;let o=this.tokens.identifierNameForToken(r),i=this.importProcessor.resolveExportBinding(o);return i?(this.tokens.appendCode(`${i} = `),this.tokens.copyToken(),!0):!1}processPostIncDec(){let n=this.tokens.currentIndex(),r=this.tokens.tokens[n],o=this.tokens.tokens[n+1];if(r.type!==t.name||r.shadowsGlobal||n>=1&&this.tokens.matches1AtIndex(n-1,t.dot))return!1;let i=this.tokens.identifierNameForToken(r),c=this.importProcessor.resolveExportBinding(i);if(!c)return!1;let p=this.tokens.rawCodeForToken(o),h=this.importProcessor.getIdentifierReplacement(i)||i;if(p==="++")this.tokens.replaceToken(`(${h} = ${c} = ${h} + 1, ${h} - 1)`);else if(p==="--")this.tokens.replaceToken(`(${h} = ${c} = ${h} - 1, ${h} + 1)`);else throw new Error(`Unexpected operator: ${p}`);return this.tokens.removeToken(),!0}processExportDefault(){let n=!0;if(this.tokens.matches4(t._export,t._default,t._function,t.name)||this.tokens.matches5(t._export,t._default,t.name,t._function,t.name)&&this.tokens.matchesContextualAtIndex(this.tokens.currentIndex()+2,l._async)){this.tokens.removeInitialToken(),this.tokens.removeToken();let r=this.processNamedFunction();this.tokens.appendCode(` exports.default = ${r};`)}else if(this.tokens.matches4(t._export,t._default,t._class,t.name)||this.tokens.matches5(t._export,t._default,t._abstract,t._class,t.name)||this.tokens.matches3(t._export,t._default,t.at)){this.tokens.removeInitialToken(),this.tokens.removeToken(),this.copyDecorators(),this.tokens.matches1(t._abstract)&&this.tokens.removeToken();let r=this.rootTransformer.processNamedClass();this.tokens.appendCode(` exports.default = ${r};`)}else if($t(this.isTypeScriptTransformEnabled,this.keepUnusedImports,this.tokens,this.declarationInfo))n=!1,this.tokens.removeInitialToken(),this.tokens.removeToken(),this.tokens.removeToken();else if(this.reactHotLoaderTransformer){let r=this.nameManager.claimFreeName("_default");this.tokens.replaceToken(`let ${r}; exports.`),this.tokens.copyToken(),this.tokens.appendCode(` = ${r} =`),this.reactHotLoaderTransformer.setExtractedDefaultExportName(r)}else this.tokens.replaceToken("exports."),this.tokens.copyToken(),this.tokens.appendCode(" =");n&&(this.hadDefaultExport=!0)}copyDecorators(){for(;this.tokens.matches1(t.at);)if(this.tokens.copyToken(),this.tokens.matches1(t.parenL))this.tokens.copyExpectedToken(t.parenL),this.rootTransformer.processBalancedCode(),this.tokens.copyExpectedToken(t.parenR);else{for(this.tokens.copyExpectedToken(t.name);this.tokens.matches1(t.dot);)this.tokens.copyExpectedToken(t.dot),this.tokens.copyExpectedToken(t.name);this.tokens.matches1(t.parenL)&&(this.tokens.copyExpectedToken(t.parenL),this.rootTransformer.processBalancedCode(),this.tokens.copyExpectedToken(t.parenR))}}processExportVar(){this.isSimpleExportVar()?this.processSimpleExportVar():this.processComplexExportVar()}isSimpleExportVar(){let n=this.tokens.currentIndex();if(n++,n++,!this.tokens.matches1AtIndex(n,t.name))return!1;for(n++;n<this.tokens.tokens.length&&this.tokens.tokens[n].isType;)n++;return!!this.tokens.matches1AtIndex(n,t.eq)}processSimpleExportVar(){this.tokens.removeInitialToken(),this.tokens.copyToken();let n=this.tokens.identifierName();for(;!this.tokens.matches1(t.eq);)this.rootTransformer.processToken();let r=this.tokens.currentToken().rhsEndIndex;if(r==null)throw new Error("Expected = token with an end index.");for(;this.tokens.currentIndex()<r;)this.rootTransformer.processToken();this.tokens.appendCode(`; exports.${n} = ${n}`)}processComplexExportVar(){this.tokens.removeInitialToken(),this.tokens.removeToken();let n=this.tokens.matches1(t.braceL);n&&this.tokens.appendCode("(");let r=0;for(;;)if(this.tokens.matches1(t.braceL)||this.tokens.matches1(t.dollarBraceL)||this.tokens.matches1(t.bracketL))r++,this.tokens.copyToken();else if(this.tokens.matches1(t.braceR)||this.tokens.matches1(t.bracketR))r--,this.tokens.copyToken();else{if(r===0&&!this.tokens.matches1(t.name)&&!this.tokens.currentToken().isType)break;if(this.tokens.matches1(t.eq)){let o=this.tokens.currentToken().rhsEndIndex;if(o==null)throw new Error("Expected = token with an end index.");for(;this.tokens.currentIndex()<o;)this.rootTransformer.processToken()}else{let o=this.tokens.currentToken();if(nn(o)){let i=this.tokens.identifierName(),c=this.importProcessor.getIdentifierReplacement(i);if(c===null)throw new Error(`Expected a replacement for ${i} in \`export var\` syntax.`);xo(o)&&(c=`${i}: ${c}`),this.tokens.replaceToken(c)}else this.rootTransformer.processToken()}}if(n){let o=this.tokens.currentToken().rhsEndIndex;if(o==null)throw new Error("Expected = token with an end index.");for(;this.tokens.currentIndex()<o;)this.rootTransformer.processToken();this.tokens.appendCode(")")}}processExportFunction(){this.tokens.replaceToken("");let n=this.processNamedFunction();this.tokens.appendCode(` exports.${n} = ${n};`)}processNamedFunction(){if(this.tokens.matches1(t._function))this.tokens.copyToken();else if(this.tokens.matches2(t.name,t._function)){if(!this.tokens.matchesContextual(l._async))throw new Error("Expected async keyword in function export.");this.tokens.copyToken(),this.tokens.copyToken()}if(this.tokens.matches1(t.star)&&this.tokens.copyToken(),!this.tokens.matches1(t.name))throw new Error("Expected identifier for exported function name.");let n=this.tokens.identifierName();if(this.tokens.copyToken(),this.tokens.currentToken().isType)for(this.tokens.removeInitialToken();this.tokens.currentToken().isType;)this.tokens.removeToken();return this.tokens.copyExpectedToken(t.parenL),this.rootTransformer.processBalancedCode(),this.tokens.copyExpectedToken(t.parenR),this.rootTransformer.processPossibleTypeRange(),this.tokens.copyExpectedToken(t.braceL),this.rootTransformer.processBalancedCode(),this.tokens.copyExpectedToken(t.braceR),n}processExportClass(){this.tokens.removeInitialToken(),this.copyDecorators(),this.tokens.matches1(t._abstract)&&this.tokens.removeToken();let n=this.rootTransformer.processNamedClass();this.tokens.appendCode(` exports.${n} = ${n};`)}processExportBindings(){this.tokens.removeInitialToken(),this.tokens.removeToken();let n=qt(this.tokens),r=[];for(;;){if(this.tokens.matches1(t.braceR)){this.tokens.removeToken();break}let o=ge(this.tokens);for(;this.tokens.currentIndex()<o.endIndex;)this.tokens.removeToken();if(!(o.isType||!n&&this.shouldElideExportedIdentifier(o.leftName))){let c=o.rightName;c==="default"?this.hadDefaultExport=!0:this.hadNamedExport=!0;let p=o.leftName,h=this.importProcessor.getIdentifierReplacement(p);r.push(`exports.${c} = ${h||p};`)}if(this.tokens.matches1(t.braceR)){this.tokens.removeToken();break}if(this.tokens.matches2(t.comma,t.braceR)){this.tokens.removeToken(),this.tokens.removeToken();break}else if(this.tokens.matches1(t.comma))this.tokens.removeToken();else throw new Error(`Unexpected token: ${JSON.stringify(this.tokens.currentToken())}`)}if(this.tokens.matchesContextual(l._from)){this.tokens.removeToken();let o=this.tokens.stringValue();this.tokens.replaceTokenTrimmingLeftWhitespace(this.importProcessor.claimImportCode(o)),Se(this.tokens)}else this.tokens.appendCode(r.join(" "));this.tokens.matches1(t.semi)&&this.tokens.removeToken()}processExportStar(){for(this.tokens.removeInitialToken();!this.tokens.matches1(t.string);)this.tokens.removeToken();let n=this.tokens.stringValue();this.tokens.replaceTokenTrimmingLeftWhitespace(this.importProcessor.claimImportCode(n)),Se(this.tokens),this.tokens.matches1(t.semi)&&this.tokens.removeToken()}shouldElideExportedIdentifier(n){return this.isTypeScriptTransformEnabled&&!this.keepUnusedImports&&!this.declarationInfo.valueDeclarations.has(n)}};var Ht=class extends H{constructor(n,r,o,i,c,p,h,g){super(),this.tokens=n,this.nameManager=r,this.helperManager=o,this.reactHotLoaderTransformer=i,this.isTypeScriptTransformEnabled=c,this.isFlowTransformEnabled=p,this.keepUnusedImports=h,this.nonTypeIdentifiers=c&&!h?cn(n,g):new Set,this.declarationInfo=c&&!h?Bt(n):On,this.injectCreateRequireForImportRequire=!!g.injectCreateRequireForImportRequire}process(){if(this.tokens.matches3(t._import,t.name,t.eq))return this.processImportEquals();if(this.tokens.matches4(t._import,t.name,t.name,t.eq)&&this.tokens.matchesContextualAtIndex(this.tokens.currentIndex()+1,l._type)){this.tokens.removeInitialToken();for(let n=0;n<7;n++)this.tokens.removeToken();return!0}if(this.tokens.matches2(t._export,t.eq))return this.tokens.replaceToken("module.exports"),!0;if(this.tokens.matches5(t._export,t._import,t.name,t.name,t.eq)&&this.tokens.matchesContextualAtIndex(this.tokens.currentIndex()+2,l._type)){this.tokens.removeInitialToken();for(let n=0;n<8;n++)this.tokens.removeToken();return!0}if(this.tokens.matches1(t._import))return this.processImport();if(this.tokens.matches2(t._export,t._default))return this.processExportDefault();if(this.tokens.matches2(t._export,t.braceL))return this.processNamedExports();if(this.tokens.matches2(t._export,t.name)&&this.tokens.matchesContextualAtIndex(this.tokens.currentIndex()+1,l._type)){if(this.tokens.removeInitialToken(),this.tokens.removeToken(),this.tokens.matches1(t.braceL)){for(;!this.tokens.matches1(t.braceR);)this.tokens.removeToken();this.tokens.removeToken()}else this.tokens.removeToken(),this.tokens.matches1(t._as)&&(this.tokens.removeToken(),this.tokens.removeToken());return this.tokens.matchesContextual(l._from)&&this.tokens.matches1AtIndex(this.tokens.currentIndex()+1,t.string)&&(this.tokens.removeToken(),this.tokens.removeToken(),Se(this.tokens)),!0}return!1}processImportEquals(){let n=this.tokens.identifierNameAtIndex(this.tokens.currentIndex()+1);return this.shouldAutomaticallyElideImportedName(n)?Ft(this.tokens):this.injectCreateRequireForImportRequire?(this.tokens.replaceToken("const"),this.tokens.copyToken(),this.tokens.copyToken(),this.tokens.replaceToken(this.helperManager.getHelperName("require"))):this.tokens.replaceToken("const"),!0}processImport(){if(this.tokens.matches2(t._import,t.parenL))return!1;let n=this.tokens.snapshot();if(this.removeImportTypeBindings()){for(this.tokens.restoreToSnapshot(n);!this.tokens.matches1(t.string);)this.tokens.removeToken();this.tokens.removeToken(),Se(this.tokens),this.tokens.matches1(t.semi)&&this.tokens.removeToken()}return!0}removeImportTypeBindings(){if(this.tokens.copyExpectedToken(t._import),this.tokens.matchesContextual(l._type)&&!this.tokens.matches1AtIndex(this.tokens.currentIndex()+1,t.comma)&&!this.tokens.matchesContextualAtIndex(this.tokens.currentIndex()+1,l._from))return!0;if(this.tokens.matches1(t.string))return this.tokens.copyToken(),!1;this.tokens.matchesContextual(l._module)&&this.tokens.matchesContextualAtIndex(this.tokens.currentIndex()+2,l._from)&&this.tokens.copyToken();let n=!1,r=!1,o=!1;if(this.tokens.matches1(t.name)&&(this.shouldAutomaticallyElideImportedName(this.tokens.identifierName())?(this.tokens.removeToken(),this.tokens.matches1(t.comma)&&this.tokens.removeToken()):(n=!0,this.tokens.copyToken(),this.tokens.matches1(t.comma)&&(o=!0,this.tokens.removeToken()))),this.tokens.matches1(t.star))this.shouldAutomaticallyElideImportedName(this.tokens.identifierNameAtRelativeIndex(2))?(this.tokens.removeToken(),this.tokens.removeToken(),this.tokens.removeToken()):(o&&this.tokens.appendCode(","),n=!0,this.tokens.copyExpectedToken(t.star),this.tokens.copyExpectedToken(t.name),this.tokens.copyExpectedToken(t.name));else if(this.tokens.matches1(t.braceL)){for(o&&this.tokens.appendCode(","),this.tokens.copyToken();!this.tokens.matches1(t.braceR);){r=!0;let i=ge(this.tokens);if(i.isType||this.shouldAutomaticallyElideImportedName(i.rightName)){for(;this.tokens.currentIndex()<i.endIndex;)this.tokens.removeToken();this.tokens.matches1(t.comma)&&this.tokens.removeToken()}else{for(n=!0;this.tokens.currentIndex()<i.endIndex;)this.tokens.copyToken();this.tokens.matches1(t.comma)&&this.tokens.copyToken()}}this.tokens.copyExpectedToken(t.braceR)}return this.keepUnusedImports?!1:this.isTypeScriptTransformEnabled?!n:this.isFlowTransformEnabled?r&&!n:!1}shouldAutomaticallyElideImportedName(n){return this.isTypeScriptTransformEnabled&&!this.keepUnusedImports&&!this.nonTypeIdentifiers.has(n)}processExportDefault(){if($t(this.isTypeScriptTransformEnabled,this.keepUnusedImports,this.tokens,this.declarationInfo))return this.tokens.removeInitialToken(),this.tokens.removeToken(),this.tokens.removeToken(),!0;if(!(this.tokens.matches4(t._export,t._default,t._function,t.name)||this.tokens.matches5(t._export,t._default,t.name,t._function,t.name)&&this.tokens.matchesContextualAtIndex(this.tokens.currentIndex()+2,l._async)||this.tokens.matches4(t._export,t._default,t._class,t.name)||this.tokens.matches5(t._export,t._default,t._abstract,t._class,t.name))&&this.reactHotLoaderTransformer){let r=this.nameManager.claimFreeName("_default");return this.tokens.replaceToken(`let ${r}; export`),this.tokens.copyToken(),this.tokens.appendCode(` ${r} =`),this.reactHotLoaderTransformer.setExtractedDefaultExportName(r),!0}return!1}processNamedExports(){if(!this.isTypeScriptTransformEnabled)return!1;this.tokens.copyExpectedToken(t._export),this.tokens.copyExpectedToken(t.braceL);let n=qt(this.tokens),r=!1;for(;!this.tokens.matches1(t.braceR);){let o=ge(this.tokens);if(o.isType||!n&&this.shouldElideExportedName(o.leftName)){for(;this.tokens.currentIndex()<o.endIndex;)this.tokens.removeToken();this.tokens.matches1(t.comma)&&this.tokens.removeToken()}else{for(r=!0;this.tokens.currentIndex()<o.endIndex;)this.tokens.copyToken();this.tokens.matches1(t.comma)&&this.tokens.copyToken()}}return this.tokens.copyExpectedToken(t.braceR),!this.keepUnusedImports&&n&&!r&&(this.tokens.removeToken(),this.tokens.removeToken(),Se(this.tokens)),!0}shouldElideExportedName(n){return this.isTypeScriptTransformEnabled&&!this.keepUnusedImports&&this.declarationInfo.typeDeclarations.has(n)&&!this.declarationInfo.valueDeclarations.has(n)}};var Vt=class extends H{constructor(n,r,o){super(),this.rootTransformer=n,this.tokens=r,this.isImportsTransformEnabled=o}process(){return this.rootTransformer.processPossibleArrowParamEnd()||this.rootTransformer.processPossibleAsyncArrowWithTypeParams()||this.rootTransformer.processPossibleTypeRange()?!0:this.tokens.matches1(t._enum)?(this.processEnum(),!0):this.tokens.matches2(t._export,t._enum)?(this.processNamedExportEnum(),!0):this.tokens.matches3(t._export,t._default,t._enum)?(this.processDefaultExportEnum(),!0):!1}processNamedExportEnum(){if(this.isImportsTransformEnabled){this.tokens.removeInitialToken();let n=this.tokens.identifierNameAtRelativeIndex(1);this.processEnum(),this.tokens.appendCode(` exports.${n} = ${n};`)}else this.tokens.copyToken(),this.processEnum()}processDefaultExportEnum(){this.tokens.removeInitialToken(),this.tokens.removeToken();let n=this.tokens.identifierNameAtRelativeIndex(1);this.processEnum(),this.isImportsTransformEnabled?this.tokens.appendCode(` exports.default = ${n};`):this.tokens.appendCode(` export default ${n};`)}processEnum(){this.tokens.replaceToken("const"),this.tokens.copyExpectedToken(t.name);let n=!1;this.tokens.matchesContextual(l._of)&&(this.tokens.removeToken(),n=this.tokens.matchesContextual(l._symbol),this.tokens.removeToken());let r=this.tokens.matches3(t.braceL,t.name,t.eq);this.tokens.appendCode(' = require("flow-enums-runtime")');let o=!n&&!r;for(this.tokens.replaceTokenTrimmingLeftWhitespace(o?".Mirrored([":"({");!this.tokens.matches1(t.braceR);){if(this.tokens.matches1(t.ellipsis)){this.tokens.removeToken();break}this.processEnumElement(n,r),this.tokens.matches1(t.comma)&&this.tokens.copyToken()}this.tokens.replaceToken(o?"]);":"});")}processEnumElement(n,r){if(n){let o=this.tokens.identifierName();this.tokens.copyToken(),this.tokens.appendCode(`: Symbol("${o}")`)}else r?(this.tokens.copyToken(),this.tokens.replaceTokenTrimmingLeftWhitespace(":"),this.tokens.copyToken()):this.tokens.replaceToken(`"${this.tokens.identifierName()}"`)}};function Ju(e){let n,r=e[0],o=1;for(;o<e.length;){let i=e[o],c=e[o+1];if(o+=2,(i==="optionalAccess"||i==="optionalCall")&&r==null)return;i==="access"||i==="optionalAccess"?(n=r,r=c(r)):(i==="call"||i==="optionalCall")&&(r=c((...p)=>r.call(n,...p)),n=void 0)}return r}var Mn="jest",Gu=["mock","unmock","enableAutomock","disableAutomock"],Wt=class e extends H{__init(){this.hoistedFunctionNames=[]}constructor(n,r,o,i){super(),this.rootTransformer=n,this.tokens=r,this.nameManager=o,this.importProcessor=i,e.prototype.__init.call(this)}process(){return this.tokens.currentToken().scopeDepth===0&&this.tokens.matches4(t.name,t.dot,t.name,t.parenL)&&this.tokens.identifierName()===Mn?Ju([this,"access",n=>n.importProcessor,"optionalAccess",n=>n.getGlobalNames,"call",n=>n(),"optionalAccess",n=>n.has,"call",n=>n(Mn)])?!1:this.extractHoistedCalls():!1}getHoistedCode(){return this.hoistedFunctionNames.length>0?this.hoistedFunctionNames.map(n=>`${n}();`).join(""):""}extractHoistedCalls(){this.tokens.removeToken();let n=!1;for(;this.tokens.matches3(t.dot,t.name,t.parenL);){let r=this.tokens.identifierNameAtIndex(this.tokens.currentIndex()+1);if(Gu.includes(r)){let i=this.nameManager.claimFreeName("__jestHoist");this.hoistedFunctionNames.push(i),this.tokens.replaceToken(`function ${i}(){${Mn}.`),this.tokens.copyToken(),this.tokens.copyToken(),this.rootTransformer.processBalancedCode(),this.tokens.copyExpectedToken(t.parenR),this.tokens.appendCode(";}"),n=!1}else n?this.tokens.copyToken():this.tokens.replaceToken(`${Mn}.`),this.tokens.copyToken(),this.tokens.copyToken(),this.rootTransformer.processBalancedCode(),this.tokens.copyExpectedToken(t.parenR),n=!0}return!0}};var zt=class extends H{constructor(n){super(),this.tokens=n}process(){if(this.tokens.matches1(t.num)){let n=this.tokens.currentTokenCode();if(n.includes("_"))return this.tokens.replaceToken(n.replace(/_/g,"")),!0}return!1}};var Xt=class extends H{constructor(n,r){super(),this.tokens=n,this.nameManager=r}process(){return this.tokens.matches2(t._catch,t.braceL)?(this.tokens.copyToken(),this.tokens.appendCode(` (${this.nameManager.claimFreeName("e")})`),!0):!1}};var Jt=class extends H{constructor(n,r){super(),this.tokens=n,this.nameManager=r}process(){if(this.tokens.matches1(t.nullishCoalescing)){let o=this.tokens.currentToken();return this.tokens.tokens[o.nullishStartIndex].isAsyncOperation?this.tokens.replaceTokenTrimmingLeftWhitespace(", async () => ("):this.tokens.replaceTokenTrimmingLeftWhitespace(", () => ("),!0}if(this.tokens.matches1(t._delete)&&this.tokens.tokenAtRelativeIndex(1).isOptionalChainStart)return this.tokens.removeInitialToken(),!0;let r=this.tokens.currentToken().subscriptStartIndex;if(r!=null&&this.tokens.tokens[r].isOptionalChainStart&&this.tokens.tokenAtRelativeIndex(-1).type!==t._super){let o=this.nameManager.claimFreeName("_"),i;if(r>0&&this.tokens.matches1AtIndex(r-1,t._delete)&&this.isLastSubscriptInChain()?i=`${o} => delete ${o}`:i=`${o} => ${o}`,this.tokens.tokens[r].isAsyncOperation&&(i=`async ${i}`),this.tokens.matches2(t.questionDot,t.parenL)||this.tokens.matches2(t.questionDot,t.lessThan))this.justSkippedSuper()&&this.tokens.appendCode(".bind(this)"),this.tokens.replaceTokenTrimmingLeftWhitespace(`, 'optionalCall', ${i}`);else if(this.tokens.matches2(t.questionDot,t.bracketL))this.tokens.replaceTokenTrimmingLeftWhitespace(`, 'optionalAccess', ${i}`);else if(this.tokens.matches1(t.questionDot))this.tokens.replaceTokenTrimmingLeftWhitespace(`, 'optionalAccess', ${i}.`);else if(this.tokens.matches1(t.dot))this.tokens.replaceTokenTrimmingLeftWhitespace(`, 'access', ${i}.`);else if(this.tokens.matches1(t.bracketL))this.tokens.replaceTokenTrimmingLeftWhitespace(`, 'access', ${i}[`);else if(this.tokens.matches1(t.parenL))this.justSkippedSuper()&&this.tokens.appendCode(".bind(this)"),this.tokens.replaceTokenTrimmingLeftWhitespace(`, 'call', ${i}(`);else throw new Error("Unexpected subscript operator in optional chain.");return!0}return!1}isLastSubscriptInChain(){let n=0;for(let r=this.tokens.currentIndex()+1;;r++){if(r>=this.tokens.tokens.length)throw new Error("Reached the end of the code while finding the end of the access chain.");if(this.tokens.tokens[r].isOptionalChainStart?n++:this.tokens.tokens[r].isOptionalChainEnd&&n--,n<0)return!0;if(n===0&&this.tokens.tokens[r].subscriptStartIndex!=null)return!1}}justSkippedSuper(){let n=0,r=this.tokens.currentIndex()-1;for(;;){if(r<0)throw new Error("Reached the start of the code while finding the start of the access chain.");if(this.tokens.tokens[r].isOptionalChainStart?n--:this.tokens.tokens[r].isOptionalChainEnd&&n++,n<0)return!1;if(n===0&&this.tokens.tokens[r].subscriptStartIndex!=null)return this.tokens.tokens[r-1].type===t._super;r--}}};var Gt=class extends H{constructor(n,r,o,i){super(),this.rootTransformer=n,this.tokens=r,this.importProcessor=o,this.options=i}process(){let n=this.tokens.currentIndex();if(this.tokens.identifierName()==="createReactClass"){let r=this.importProcessor&&this.importProcessor.getIdentifierReplacement("createReactClass");return r?this.tokens.replaceToken(`(0, ${r})`):this.tokens.copyToken(),this.tryProcessCreateClassCall(n),!0}if(this.tokens.matches3(t.name,t.dot,t.name)&&this.tokens.identifierName()==="React"&&this.tokens.identifierNameAtIndex(this.tokens.currentIndex()+2)==="createClass"){let r=this.importProcessor&&this.importProcessor.getIdentifierReplacement("React")||"React";return r?(this.tokens.replaceToken(r),this.tokens.copyToken(),this.tokens.copyToken()):(this.tokens.copyToken(),this.tokens.copyToken(),this.tokens.copyToken()),this.tryProcessCreateClassCall(n),!0}return!1}tryProcessCreateClassCall(n){let r=this.findDisplayName(n);r&&this.classNeedsDisplayName()&&(this.tokens.copyExpectedToken(t.parenL),this.tokens.copyExpectedToken(t.braceL),this.tokens.appendCode(`displayName: '${r}',`),this.rootTransformer.processBalancedCode(),this.tokens.copyExpectedToken(t.braceR),this.tokens.copyExpectedToken(t.parenR))}findDisplayName(n){return n<2?null:this.tokens.matches2AtIndex(n-2,t.name,t.eq)?this.tokens.identifierNameAtIndex(n-2):n>=2&&this.tokens.tokens[n-2].identifierRole===_.ObjectKey?this.tokens.identifierNameAtIndex(n-2):this.tokens.matches2AtIndex(n-2,t._export,t._default)?this.getDisplayNameFromFilename():null}getDisplayNameFromFilename(){let r=(this.options.filePath||"unknown").split("/"),o=r[r.length-1],i=o.lastIndexOf("."),c=i===-1?o:o.slice(0,i);return c==="index"&&r[r.length-2]?r[r.length-2]:c}classNeedsDisplayName(){let n=this.tokens.currentIndex();if(!this.tokens.matches2(t.parenL,t.braceL))return!1;let r=n+1,o=this.tokens.tokens[r].contextId;if(o==null)throw new Error("Expected non-null context ID on object open-brace.");for(;n<this.tokens.tokens.length;n++){let i=this.tokens.tokens[n];if(i.type===t.braceR&&i.contextId===o){n++;break}if(this.tokens.identifierNameAtIndex(n)==="displayName"&&this.tokens.tokens[n].identifierRole===_.ObjectKey&&i.contextId===o)return!1}if(n===this.tokens.tokens.length)throw new Error("Unexpected end of input when processing React class.");return this.tokens.matches1AtIndex(n,t.parenR)||this.tokens.matches2AtIndex(n,t.comma,t.parenR)}};var Kt=class e extends H{__init(){this.extractedDefaultExportName=null}constructor(n,r){super(),this.tokens=n,this.filePath=r,e.prototype.__init.call(this)}setExtractedDefaultExportName(n){this.extractedDefaultExportName=n}getPrefixCode(){return`
      (function () {
        var enterModule = require('react-hot-loader').enterModule;
        enterModule && enterModule(module);
      })();`.replace(/\s+/g," ").trim()}getSuffixCode(){let n=new Set;for(let o of this.tokens.tokens)!o.isType&&rn(o)&&o.identifierRole!==_.ImportDeclaration&&n.add(this.tokens.identifierNameForToken(o));let r=Array.from(n).map(o=>({variableName:o,uniqueLocalName:o}));return this.extractedDefaultExportName&&r.push({variableName:this.extractedDefaultExportName,uniqueLocalName:"default"}),`
;(function () {
  var reactHotLoader = require('react-hot-loader').default;
  var leaveModule = require('react-hot-loader').leaveModule;
  if (!reactHotLoader) {
    return;
  }
${r.map(({variableName:o,uniqueLocalName:i})=>`  reactHotLoader.register(${o}, "${i}", ${JSON.stringify(this.filePath||"")});`).join(`
`)}
  leaveModule(module);
})();`}process(){return!1}};var Ku=new Set(["break","case","catch","class","const","continue","debugger","default","delete","do","else","export","extends","finally","for","function","if","import","in","instanceof","new","return","super","switch","this","throw","try","typeof","var","void","while","with","yield","enum","implements","interface","let","package","private","protected","public","static","await","false","null","true"]);function jn(e){if(e.length===0||!be[e.charCodeAt(0)])return!1;for(let n=1;n<e.length;n++)if(!Z[e.charCodeAt(n)])return!1;return!Ku.has(e)}var Yt=class extends H{constructor(n,r,o){super(),this.rootTransformer=n,this.tokens=r,this.isImportsTransformEnabled=o}process(){return this.rootTransformer.processPossibleArrowParamEnd()||this.rootTransformer.processPossibleAsyncArrowWithTypeParams()||this.rootTransformer.processPossibleTypeRange()?!0:this.tokens.matches1(t._public)||this.tokens.matches1(t._protected)||this.tokens.matches1(t._private)||this.tokens.matches1(t._abstract)||this.tokens.matches1(t._readonly)||this.tokens.matches1(t._override)||this.tokens.matches1(t.nonNullAssertion)?(this.tokens.removeInitialToken(),!0):this.tokens.matches1(t._enum)||this.tokens.matches2(t._const,t._enum)?(this.processEnum(),!0):this.tokens.matches2(t._export,t._enum)||this.tokens.matches3(t._export,t._const,t._enum)?(this.processEnum(!0),!0):!1}processEnum(n=!1){for(this.tokens.removeInitialToken();this.tokens.matches1(t._const)||this.tokens.matches1(t._enum);)this.tokens.removeToken();let r=this.tokens.identifierName();this.tokens.removeToken(),n&&!this.isImportsTransformEnabled&&this.tokens.appendCode("export "),this.tokens.appendCode(`var ${r}; (function (${r})`),this.tokens.copyExpectedToken(t.braceL),this.processEnumBody(r),this.tokens.copyExpectedToken(t.braceR),n&&this.isImportsTransformEnabled?this.tokens.appendCode(`)(${r} || (exports.${r} = ${r} = {}));`):this.tokens.appendCode(`)(${r} || (${r} = {}));`)}processEnumBody(n){let r=null;for(;!this.tokens.matches1(t.braceR);){let{nameStringCode:o,variableName:i}=this.extractEnumKeyInfo(this.tokens.currentToken());this.tokens.removeInitialToken(),this.tokens.matches3(t.eq,t.string,t.comma)||this.tokens.matches3(t.eq,t.string,t.braceR)?this.processStringLiteralEnumMember(n,o,i):this.tokens.matches1(t.eq)?this.processExplicitValueEnumMember(n,o,i):this.processImplicitValueEnumMember(n,o,i,r),this.tokens.matches1(t.comma)&&this.tokens.removeToken(),i!=null?r=i:r=`${n}[${o}]`}}extractEnumKeyInfo(n){if(n.type===t.name){let r=this.tokens.identifierNameForToken(n);return{nameStringCode:`"${r}"`,variableName:jn(r)?r:null}}else if(n.type===t.string){let r=this.tokens.stringValueForToken(n);return{nameStringCode:this.tokens.code.slice(n.start,n.end),variableName:jn(r)?r:null}}else throw new Error("Expected name or string at beginning of enum element.")}processStringLiteralEnumMember(n,r,o){o!=null?(this.tokens.appendCode(`const ${o}`),this.tokens.copyToken(),this.tokens.copyToken(),this.tokens.appendCode(`; ${n}[${r}] = ${o};`)):(this.tokens.appendCode(`${n}[${r}]`),this.tokens.copyToken(),this.tokens.copyToken(),this.tokens.appendCode(";"))}processExplicitValueEnumMember(n,r,o){let i=this.tokens.currentToken().rhsEndIndex;if(i==null)throw new Error("Expected rhsEndIndex on enum assign.");if(o!=null){for(this.tokens.appendCode(`const ${o}`),this.tokens.copyToken();this.tokens.currentIndex()<i;)this.rootTransformer.processToken();this.tokens.appendCode(`; ${n}[${n}[${r}] = ${o}] = ${r};`)}else{for(this.tokens.appendCode(`${n}[${n}[${r}]`),this.tokens.copyToken();this.tokens.currentIndex()<i;)this.rootTransformer.processToken();this.tokens.appendCode(`] = ${r};`)}}processImplicitValueEnumMember(n,r,o,i){let c=i!=null?`${i} + 1`:"0";o!=null&&(this.tokens.appendCode(`const ${o} = ${c}; `),c=o),this.tokens.appendCode(`${n}[${n}[${r}] = ${c}] = ${r};`)}};var Qt=class e{__init(){this.transformers=[]}__init2(){this.generatedVariables=[]}constructor(n,r,o,i){e.prototype.__init.call(this),e.prototype.__init2.call(this),this.nameManager=n.nameManager,this.helperManager=n.helperManager;let{tokenProcessor:c,importProcessor:p}=n;this.tokens=c,this.isImportsTransformEnabled=r.includes("imports"),this.isReactHotLoaderTransformEnabled=r.includes("react-hot-loader"),this.disableESTransforms=!!i.disableESTransforms,i.disableESTransforms||(this.transformers.push(new Jt(c,this.nameManager)),this.transformers.push(new zt(c)),this.transformers.push(new Xt(c,this.nameManager))),r.includes("jsx")&&(i.jsxRuntime!=="preserve"&&this.transformers.push(new xt(this,c,p,this.nameManager,i)),this.transformers.push(new Gt(this,c,p,i)));let h=null;if(r.includes("react-hot-loader")){if(!i.filePath)throw new Error("filePath is required when using the react-hot-loader transform.");h=new Kt(c,i.filePath),this.transformers.push(h)}if(r.includes("imports")){if(p===null)throw new Error("Expected non-null importProcessor with imports transform enabled.");this.transformers.push(new Ut(this,c,p,this.nameManager,this.helperManager,h,o,!!i.enableLegacyTypeScriptModuleInterop,r.includes("typescript"),r.includes("flow"),!!i.preserveDynamicImport,!!i.keepUnusedImports))}else this.transformers.push(new Ht(c,this.nameManager,this.helperManager,h,r.includes("typescript"),r.includes("flow"),!!i.keepUnusedImports,i));r.includes("flow")&&this.transformers.push(new Vt(this,c,r.includes("imports"))),r.includes("typescript")&&this.transformers.push(new Yt(this,c,r.includes("imports"))),r.includes("jest")&&this.transformers.push(new Wt(this,c,this.nameManager,p))}transform(){this.tokens.reset(),this.processBalancedCode();let r=this.isImportsTransformEnabled?'"use strict";':"";for(let p of this.transformers)r+=p.getPrefixCode();r+=this.helperManager.emitHelpers(),r+=this.generatedVariables.map(p=>` var ${p};`).join("");for(let p of this.transformers)r+=p.getHoistedCode();let o="";for(let p of this.transformers)o+=p.getSuffixCode();let i=this.tokens.finish(),{code:c}=i;if(c.startsWith("#!")){let p=c.indexOf(`
`);return p===-1&&(p=c.length,c+=`
`),{code:c.slice(0,p+1)+r+c.slice(p+1)+o,mappings:this.shiftMappings(i.mappings,r.length)}}else return{code:r+c+o,mappings:this.shiftMappings(i.mappings,r.length)}}processBalancedCode(){let n=0,r=0;for(;!this.tokens.isAtEnd();){if(this.tokens.matches1(t.braceL)||this.tokens.matches1(t.dollarBraceL))n++;else if(this.tokens.matches1(t.braceR)){if(n===0)return;n--}if(this.tokens.matches1(t.parenL))r++;else if(this.tokens.matches1(t.parenR)){if(r===0)return;r--}this.processToken()}}processToken(){if(this.tokens.matches1(t._class)){this.processClass();return}for(let n of this.transformers)if(n.process())return;this.tokens.copyToken()}processNamedClass(){if(!this.tokens.matches2(t._class,t.name))throw new Error("Expected identifier for exported class name.");let n=this.tokens.identifierNameAtIndex(this.tokens.currentIndex()+1);return this.processClass(),n}processClass(){let n=co(this,this.tokens,this.nameManager,this.disableESTransforms),r=(n.headerInfo.isExpression||!n.headerInfo.className)&&n.staticInitializerNames.length+n.instanceInitializerNames.length>0,o=n.headerInfo.className;r&&(o=this.nameManager.claimFreeName("_class"),this.generatedVariables.push(o),this.tokens.appendCode(` (${o} =`));let c=this.tokens.currentToken().contextId;if(c==null)throw new Error("Expected class to have a context ID.");for(this.tokens.copyExpectedToken(t._class);!this.tokens.matchesContextIdAndLabel(t.braceL,c);)this.processToken();this.processClassBody(n,o);let p=n.staticInitializerNames.map(h=>`${o}.${h}()`);r?this.tokens.appendCode(`, ${p.map(h=>`${h}, `).join("")}${o})`):n.staticInitializerNames.length>0&&this.tokens.appendCode(` ${p.map(h=>`${h};`).join(" ")}`)}processClassBody(n,r){let{headerInfo:o,constructorInsertPos:i,constructorInitializerStatements:c,fields:p,instanceInitializerNames:h,rangesToRemove:g}=n,y=0,x=0,v=this.tokens.currentToken().contextId;if(v==null)throw new Error("Expected non-null context ID on class.");this.tokens.copyExpectedToken(t.braceL),this.isReactHotLoaderTransformEnabled&&this.tokens.appendCode("__reactstandin__regenerateByEval(key, code) {this[key] = eval(code);}");let T=c.length+h.length>0;if(i===null&&T){let E=this.makeConstructorInitCode(c,h,r);if(o.hasSuperclass){let O=this.nameManager.claimFreeName("args");this.tokens.appendCode(`constructor(...${O}) { super(...${O}); ${E}; }`)}else this.tokens.appendCode(`constructor() { ${E}; }`)}for(;!this.tokens.matchesContextIdAndLabel(t.braceR,v);)if(y<p.length&&this.tokens.currentIndex()===p[y].start){let E=!1;for(this.tokens.matches1(t.bracketL)?this.tokens.copyTokenWithPrefix(`${p[y].initializerName}() {this`):this.tokens.matches1(t.string)||this.tokens.matches1(t.num)?(this.tokens.copyTokenWithPrefix(`${p[y].initializerName}() {this[`),E=!0):this.tokens.copyTokenWithPrefix(`${p[y].initializerName}() {this.`);this.tokens.currentIndex()<p[y].end;)E&&this.tokens.currentIndex()===p[y].equalsIndex&&this.tokens.appendCode("]"),this.processToken();this.tokens.appendCode("}"),y++}else if(x<g.length&&this.tokens.currentIndex()>=g[x].start){for(this.tokens.currentIndex()<g[x].end&&this.tokens.removeInitialToken();this.tokens.currentIndex()<g[x].end;)this.tokens.removeToken();x++}else this.tokens.currentIndex()===i?(this.tokens.copyToken(),T&&this.tokens.appendCode(`;${this.makeConstructorInitCode(c,h,r)};`),this.processToken()):this.processToken();this.tokens.copyExpectedToken(t.braceR)}makeConstructorInitCode(n,r,o){return[...n,...r.map(i=>`${o}.prototype.${i}.call(this)`)].join(";")}processPossibleArrowParamEnd(){if(this.tokens.matches2(t.parenR,t.colon)&&this.tokens.tokenAtRelativeIndex(1).isType){let n=this.tokens.currentIndex()+1;for(;this.tokens.tokens[n].isType;)n++;if(this.tokens.matches1AtIndex(n,t.arrow)){for(this.tokens.removeInitialToken();this.tokens.currentIndex()<n;)this.tokens.removeToken();return this.tokens.replaceTokenTrimmingLeftWhitespace(") =>"),!0}}return!1}processPossibleAsyncArrowWithTypeParams(){if(!this.tokens.matchesContextual(l._async)&&!this.tokens.matches1(t._async))return!1;let n=this.tokens.tokenAtRelativeIndex(1);if(n.type!==t.lessThan||!n.isType)return!1;let r=this.tokens.currentIndex()+1;for(;this.tokens.tokens[r].isType;)r++;if(this.tokens.matches1AtIndex(r,t.parenL)){for(this.tokens.replaceToken("async ("),this.tokens.removeInitialToken();this.tokens.currentIndex()<r;)this.tokens.removeToken();return this.tokens.removeToken(),this.processBalancedCode(),this.processToken(),!0}return!1}processPossibleTypeRange(){if(this.tokens.currentToken().isType){for(this.tokens.removeInitialToken();this.tokens.currentToken().isType;)this.tokens.removeToken();return!0}return!1}shiftMappings(n,r){for(let o=0;o<n.length;o++){let i=n[o];i!==void 0&&(n[o]=i+r)}return n}};var Yu=Jr(M1());function lo(e){let n=new Set;for(let r=0;r<e.tokens.length;r++)e.matches1AtIndex(r,t._import)&&!e.matches3AtIndex(r,t._import,t.name,t.eq)&&Qu(e,r,n);return n}function Qu(e,n,r){n++,!e.matches1AtIndex(n,t.parenL)&&(e.matches1AtIndex(n,t.name)&&(r.add(e.identifierNameAtIndex(n)),n++,e.matches1AtIndex(n,t.comma)&&n++),e.matches1AtIndex(n,t.star)&&(n+=2,r.add(e.identifierNameAtIndex(n)),n++),e.matches1AtIndex(n,t.braceL)&&(n++,Zu(e,n,r)))}function Zu(e,n,r){for(;;){if(e.matches1AtIndex(n,t.braceR))return;let o=ge(e,n);if(n=o.endIndex,o.isType||r.add(o.rightName),e.matches2AtIndex(n,t.comma,t.braceR))return;if(e.matches1AtIndex(n,t.braceR))return;if(e.matches1AtIndex(n,t.comma))n++;else throw new Error(`Unexpected token: ${JSON.stringify(e.tokens[n])}`)}}function j1(e,n){ti(n);try{let r=ep(e,n),i=new Qt(r,n.transforms,!!n.enableLegacyBabel5ModuleInterop,n).transform(),c={code:i.code};if(n.sourceMapOptions){if(!n.filePath)throw new Error("filePath must be specified when generating a source map.");c={...c,sourceMap:ds(i,n.filePath,n.sourceMapOptions,e,r.tokenProcessor.tokens)}}return c}catch(r){throw n.filePath&&(r.message=`Error transforming ${n.filePath}: ${r.message}`),r}}function ep(e,n){let r=n.transforms.includes("jsx"),o=n.transforms.includes("typescript"),i=n.transforms.includes("flow"),c=n.disableESTransforms===!0,p=R1(e,r,o,i),h=p.tokens,g=p.scopes,y=new vt(e,h),x=new un(y),v=new jt(e,h,i,c,x),T=!!n.enableLegacyTypeScriptModuleInterop,E=null;return n.transforms.includes("imports")?(E=new bt(y,v,T,n,n.transforms.includes("typescript"),!!n.keepUnusedImports,x),E.preprocessTokens(),pn(v,g,E.getGlobalNames()),n.transforms.includes("typescript")&&!n.keepUnusedImports&&E.pruneTypeOnlyImports()):n.transforms.includes("typescript")&&!n.keepUnusedImports&&pn(v,g,lo(v)),{tokenProcessor:v,scopes:g,nameManager:y,importProcessor:E,helperManager:x}}var F1={metadata:"music.metadata",streaming:"music.stream",lyrics:"lyrics",dashboard:"music.charts",discovery:"music.recommend",playlists:"music.metadata"},tp=["Queue","Playback","Favorites","Playlists","Dashboard","Discovery","Events","Shell","Navigation","Metadata","Streaming","Lyrics"],np=["www.youtube.com","music.youtube.com","*.googlevideo.com"],rp=["index.ts","index.tsx","index.js","src/index.ts","src/index.tsx","src/index.js","dist/index.js"],sp=e=>/\.(tsx?|jsx?|mjs|cjs|json)$/.test(e)&&!e.endsWith(".d.ts")&&!/\.(test|spec)\.[cm]?[jt]sx?$/.test(e)&&!/(^|\/)(test|tests|__tests__|__mocks__|node_modules)\//.test(e)&&!/(^|\/)(package-lock|tsconfig[^/]*|package)\.json$/.test(e),op=e=>e.toLowerCase().replace(/^@/,"").replace(/[^a-z0-9]+/g,"-").replace(/^-+|-+$/g,"").slice(0,80)||"plugin",ip=5,ap=/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$/,cp=new Set(["localhost","example.com","example.org","www.w3.org"]),lp=e=>{let n=new Set;for(let r of e)for(let[,o]of r.matchAll(/https?:\/\/([a-zA-Z0-9-]+(?:\.[a-zA-Z0-9-]+)+)/g)){let i=(o??"").toLowerCase();cp.has(i)||n.add(i)}return[...n].sort()},up=(e,n)=>{if(e.endsWith(".json"))return`module.exports = ${n.trim()||"null"};`;let r=/\.[jt]sx$/.test(e);return j1(n,{transforms:[.../\.tsx?$/.test(e)?["typescript"]:[],...r?["jsx"]:[],"imports"],jsxRuntime:"automatic",production:!0,filePath:e}).code},B1=(e,n)=>{let r=Object.keys(e).filter(N=>N==="package.json"||N.endsWith("/package.json")).filter(N=>!N.includes("node_modules/")).sort((N,J)=>N.split("/").length-J.split("/").length)[0];if(!r)throw new Error("This is not a Nuclear plugin: there is no package.json");let o=r.slice(0,-12),i=JSON.parse(e[r]);if(!i.name)throw new Error("package.json has no name");let c={};for(let[N,J]of Object.entries(e)){if(!N.startsWith(o))continue;let ae=N.slice(o.length);sp(ae)&&(c[ae]=J)}let p=[i.main?.replace(/^\.\//,""),...rp].find(N=>!!N&&N in c);if(!p)throw new Error(`Can't find the plugin's entry file (${i.main??"index.ts"})`);let h=i.nuclear?.categories??(i.nuclear?.category?[i.nuclear.category]:[]),g=Object.values(c),y=[...new Set(g.flatMap(N=>[...N.matchAll(/\bkind\s*:\s*['"](metadata|streaming|lyrics|dashboard|discovery|playlists)['"]/g)].map(([,J])=>J)))],x=g.some(N=>/\.Ytdlp\s*\./.test(N)),v={supported:[...new Set([...h,...y].flatMap(N=>F1[N]??[]))],unsupportedCategories:h.filter(N=>!(N in F1)),stubbedApis:tp.filter(N=>g.some(J=>new RegExp(`\\.${N}\\s*\\.`).test(J))),uiImports:[...new Set(g.flatMap(N=>[...N.matchAll(/from\s+['"](react(?:\/[\w-]+)?|@nuclearplayer\/ui)['"]/g)].map(([,J])=>J)))].sort(),hosts:[...new Set([...lp(g),...x?np:[]])].sort(),modules:Object.keys(c).length};if(v.supported.length===0){let N=h.length>0?h.join(", "):"no provider categories";throw new Error(`Raygun can't run this Nuclear plugin yet: it provides ${N}`)}let T=Object.entries(c).map(([N,J])=>`${JSON.stringify(N)}: function (exports, require, module, document, window) {
${up(N,J)}
}`).join(`,
`),E={id:i.name,name:i.nuclear?.displayName??i.name,version:i.version??"0.0.0",categories:h},O=[`// Nuclear plugin ${i.name} ${i.version??""}, converted for Raygun by @raygun/nuclear-compat.`,n,"(function () {",`var definitions = {
${T}
};`,`__nuclearShim.run({ entry: ${JSON.stringify(p)}, definitions: definitions, plugin: ${JSON.stringify(E)} });`,"})();",""].join(`
`),q=typeof i.author=="string"?i.author:i.author?.name;return{manifest:{id:`nuclear.${op(i.name)}`,name:(i.nuclear?.displayName??i.name).slice(0,80),version:i.version&&ap.test(i.version)?i.version:"0.0.0",apiVersion:1,runtime:"js",entry:"dist/index.js",description:i.description??null,author:q??null,provides:v.supported,permissions:{net:v.hosts},contentRating:"safe",nuclear:{package:i.name,converter:ip,report:v}},bundle:O,report:v}};globalThis.__convert=e=>{try{return JSON.stringify(B1(JSON.parse(e),po))}catch(n){return JSON.stringify({error:n instanceof Error?n.message:String(n)})}};})();
